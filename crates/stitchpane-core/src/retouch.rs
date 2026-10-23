//! Smear strokes applied to pixels (SMEAR-002 to SMEAR-005).

use std::hash::{DefaultHasher, Hash, Hasher};

use image::RgbaImage;

use crate::model::{SmearKind, Stroke};
use crate::panic_free::{self, Clamped};

/// Applies `stroke` to `image`. `map` converts the stroke's source-pixel coordinates to
/// pixels of `image`, which has `scale` pixels per source pixel. Pixels outside the image
/// are neither read nor written.
pub fn apply(
    image: &mut RgbaImage,
    stroke: &Stroke,
    map: impl Fn([f64; 2]) -> [f64; 2],
    scale: f64,
) {
    let radius = stroke.size * scale / 2.0;
    if radius < 0.5 || stroke.points.is_empty() {
        return;
    }
    // Smudge stamps the color it carries, so its dabs need to be closer together to
    // avoid a scalloped texture.
    let spacing = match stroke.kind {
        SmearKind::Smudge => radius / 12.0,
        SmearKind::Blur | SmearKind::Push => radius / 4.0,
    };
    let dabs = dabs(stroke.points.iter().map(|&p| map(p)), spacing.max(0.5));
    let strength = stroke.strength as f32;
    match stroke.kind {
        SmearKind::Smudge => smudge(image, &dabs, radius, strength),
        SmearKind::Blur => blur(image, &dabs, radius, strength),
        SmearKind::Push => push(image, &dabs, radius, strength),
    }
}

/// A hash of strokes, to tell apart pixels made with different strokes.
pub fn strokes_hash<'a>(strokes: impl IntoIterator<Item = &'a Stroke>) -> u64 {
    let mut hasher = DefaultHasher::new();
    for stroke in strokes {
        stroke.kind.hash(&mut hasher);
        stroke.size.to_bits().hash(&mut hasher);
        stroke.strength.to_bits().hash(&mut hasher);
        for p in &stroke.points {
            p[0].to_bits().hash(&mut hasher);
            p[1].to_bits().hash(&mut hasher);
        }
    }
    hasher.finish()
}

/// Brush positions along the path, `spacing` apart.
fn dabs(mut path: impl Iterator<Item = [f64; 2]>, spacing: f64) -> Vec<[f64; 2]> {
    let Some(mut at) = path.next() else {
        return Vec::new();
    };
    let mut dabs = vec![at];
    for p in path {
        loop {
            let d = (p[0] - at[0]).hypot(p[1] - at[1]);
            if d < spacing {
                break;
            }
            let t = spacing / d;
            at = [at[0] + (p[0] - at[0]) * t, at[1] + (p[1] - at[1]) * t];
            dabs.push(at);
        }
    }
    dabs
}

/// Brush weight at squared distance `d2` from the center: 1 at the center, fading to 0
/// at the edge (SMEAR-003).
fn falloff(d2: f64, radius: f64) -> f32 {
    let t = d2 / (radius * radius);
    if t >= 1.0 {
        0.0
    } else {
        ((1.0 - t) * (1.0 - t)) as f32
    }
}

fn read(image: &RgbaImage, x: i64, y: i64) -> Option<[f32; 4]> {
    let (x, y) = (u32::try_from(x).ok()?, u32::try_from(y).ok()?);
    Some(image.get_pixel_checked(x, y)?.0.map(f32::from))
}

fn write(image: &mut RgbaImage, x: i64, y: i64, color: [f32; 4]) {
    let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) else {
        return;
    };
    if let Some(pixel) = image.get_pixel_mut_checked(x, y) {
        pixel.0 = color.map(|c| c.clamped(0.0, 255.0).round() as u8);
    }
}

fn mix(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let mut out = a;
    for (o, b) in out.iter_mut().zip(b) {
        *o += (b - *o) * t;
    }
    out
}

/// The pixel at column `i`, row `j` of a square `side` × `side` area, or transparent
/// black outside it.
fn area_at(area: &[[f32; 4]], side: usize, i: usize, j: usize) -> [f32; 4] {
    if i >= side {
        return [0.0; 4];
    }
    let index = j.saturating_mul(side).saturating_add(i);
    area.get(index).copied().unwrap_or_default()
}

/// Copies the `side` × `side` area of `image` whose top left is (`x0`, `y0`), clamping
/// reads to the image.
fn copy_area(image: &RgbaImage, x0: i64, y0: i64, side: usize) -> Vec<[f32; 4]> {
    let (w, h) = (i64::from(image.width()), i64::from(image.height()));
    let mut area = vec![[0f32; 4]; side.saturating_mul(side)];
    for (j, row) in panic_free::chunks_mut(&mut area, side).enumerate() {
        for (i, p) in row.iter_mut().enumerate() {
            let x = x0.saturating_add(i as i64).min(w.saturating_sub(1)).max(0);
            let y = y0.saturating_add(j as i64).min(h.saturating_sub(1)).max(0);
            *p = read(image, x, y).unwrap_or_default();
        }
    }
    area
}

/// Each brush pixel within the image: (x, y, offset index, weight).
fn brush_pixels(
    image: &RgbaImage,
    center: [f64; 2],
    radius: f64,
) -> impl Iterator<Item = (i64, i64, usize, f32)> {
    let r = radius.ceil() as i64;
    let side = r.saturating_mul(2).saturating_add(1);
    let (cx, cy) = (center[0].floor() as i64, center[1].floor() as i64);
    let (w, h) = (i64::from(image.width()), i64::from(image.height()));
    (r.saturating_neg()..=r).flat_map(move |oy| {
        (r.saturating_neg()..=r).filter_map(move |ox| {
            let (x, y) = (cx.saturating_add(ox), cy.saturating_add(oy));
            if x < 0 || y < 0 || x >= w || y >= h {
                return None;
            }
            let (dx, dy) = (x as f64 + 0.5 - center[0], y as f64 + 0.5 - center[1]);
            let weight = falloff(dx * dx + dy * dy, radius);
            let index = oy
                .saturating_add(r)
                .saturating_mul(side)
                .saturating_add(ox)
                .saturating_add(r);
            (weight > 0.0).then_some((x, y, index as usize, weight))
        })
    })
}

/// Picks up color at the first dab and drags it along, mixing in what it passes over.
fn smudge(image: &mut RgbaImage, dabs: &[[f64; 2]], radius: f64, strength: f32) {
    let r = radius.ceil() as i64;
    let side = r.saturating_mul(2).saturating_add(1) as usize;
    let mut carry: Vec<Option<[f32; 4]>> = vec![None; side.saturating_mul(side)];
    for (index, &dab) in dabs.iter().enumerate() {
        let pixels: Vec<_> = brush_pixels(image, dab, radius).collect();
        for (x, y, i, weight) in pixels {
            let (Some(here), Some(slot)) = (read(image, x, y), carry.get_mut(i)) else {
                continue;
            };
            let carried = slot.unwrap_or(here);
            if index > 0 {
                write(image, x, y, mix(here, carried, weight * strength));
            }
            // Strength is how much of the carried color survives each step.
            *slot = Some(mix(here, carried, strength));
        }
    }
}

/// Softens the pixels under the brush with a box blur.
fn blur(image: &mut RgbaImage, dabs: &[[f64; 2]], radius: f64, strength: f32) {
    let k = (radius / 4.0).round().max(1.0) as i64;
    let r = radius.ceil() as i64;
    let reach = r.saturating_add(k);
    for &dab in dabs {
        let (cx, cy) = (dab[0].floor() as i64, dab[1].floor() as i64);
        let (x0, y0) = (cx.saturating_sub(reach), cy.saturating_sub(reach));
        let side = reach.saturating_mul(2).saturating_add(1) as usize;
        let area = copy_area(image, x0, y0, side);
        let blurred = box_blur(&box_blur(&area, side, k, true), side, k, false);
        let pixels: Vec<_> = brush_pixels(image, dab, radius).collect();
        for (x, y, _, weight) in pixels {
            let i = x.saturating_sub(x0) as usize;
            let j = y.saturating_sub(y0) as usize;
            let here = area_at(&area, side, i, j);
            let soft = area_at(&blurred, side, i, j);
            write(image, x, y, mix(here, soft, weight * strength));
        }
    }
}

fn box_blur(area: &[[f32; 4]], side: usize, k: i64, horizontal: bool) -> Vec<[f32; 4]> {
    let mut out = vec![[0f32; 4]; area.len()];
    let n = 2.0 * k as f32 + 1.0;
    let last = (side as i64).saturating_sub(1);
    for (j, row) in panic_free::chunks_mut(&mut out, side).enumerate() {
        for (i, p) in row.iter_mut().enumerate() {
            let mut sum = [0f32; 4];
            for d in k.saturating_neg()..=k {
                let (si, sj) = if horizontal {
                    ((i as i64).saturating_add(d).min(last).max(0) as usize, j)
                } else {
                    (i, (j as i64).saturating_add(d).min(last).max(0) as usize)
                };
                for (s, v) in sum.iter_mut().zip(area_at(area, side, si, sj)) {
                    *s += v;
                }
            }
            *p = sum.map(|v| v / n);
        }
    }
    out
}

/// Moves pixels under the brush along the stroke.
fn push(image: &mut RgbaImage, dabs: &[[f64; 2]], radius: f64, strength: f32) {
    for (&from, &to) in dabs.iter().zip(dabs.iter().skip(1)) {
        let v = [to[0] - from[0], to[1] - from[1]];
        // Copy the area the brush reads from, clamping reads to the image.
        let reach = (radius.ceil() as i64)
            .saturating_add(v[0].abs().max(v[1].abs()).ceil() as i64)
            .saturating_add(2);
        let x0 = (to[0].floor() as i64).saturating_sub(reach);
        let y0 = (to[1].floor() as i64).saturating_sub(reach);
        let side = reach.saturating_mul(2).saturating_add(1) as usize;
        let area = copy_area(image, x0, y0, side);
        let pixels: Vec<_> = brush_pixels(image, to, radius).collect();
        for (x, y, _, weight) in pixels {
            let t = f64::from(weight * strength);
            let sx = x as f64 + 0.5 - v[0] * t - x0 as f64;
            let sy = y as f64 + 0.5 - v[1] * t - y0 as f64;
            write(image, x, y, bilinear(&area, side, sx, sy));
        }
    }
}

/// Bilinear sample of a square `side` × `side` area at (`x`, `y`), clamped to the area.
fn bilinear(area: &[[f32; 4]], side: usize, x: f64, y: f64) -> [f32; 4] {
    let (gx, gy) = (x - 0.5, y - 0.5);
    let (x0, y0) = (gx.floor() as i64, gy.floor() as i64);
    let (tx, ty) = ((gx - gx.floor()) as f32, (gy - gy.floor()) as f32);
    let last = (side as i64).saturating_sub(1);
    let at = |x: i64, y: i64| {
        area_at(
            area,
            side,
            x.min(last).max(0) as usize,
            y.min(last).max(0) as usize,
        )
    };
    let (x1, y1) = (x0.saturating_add(1), y0.saturating_add(1));
    let top = mix(at(x0, y0), at(x1, y0), tx);
    let bottom = mix(at(x0, y1), at(x1, y1), tx);
    mix(top, bottom, ty)
}

#[cfg(test)]
#[allow(
    clippy::disallowed_macros,
    clippy::disallowed_methods,
    reason = "tests may panic (RUST-001)"
)]
mod tests {
    use super::*;

    fn halves() -> RgbaImage {
        RgbaImage::from_fn(40, 20, |x, _| {
            image::Rgba(if x < 20 {
                [255, 0, 0, 255]
            } else {
                [0, 0, 255, 255]
            })
        })
    }

    fn stroke(kind: SmearKind, points: Vec<[f64; 2]>) -> Stroke {
        Stroke {
            kind,
            size: 10.0,
            strength: 0.8,
            points,
        }
    }

    #[test]
    fn smudge_drags_color_along_the_stroke() {
        // spec: SMEAR-002
        let mut image = halves();
        let s = stroke(SmearKind::Smudge, vec![[10.0, 10.0], [30.0, 10.0]]);
        apply(&mut image, &s, |p| p, 1.0);
        let p = image.get_pixel(30, 10).0;
        assert!(p[0] > 60, "red was dragged into blue: {p:?}");
        assert_eq!(image.get_pixel(30, 0).0, [0, 0, 255, 255]);
    }

    #[test]
    fn blur_mixes_the_edge() {
        // spec: SMEAR-002
        let mut image = halves();
        let s = stroke(SmearKind::Blur, vec![[20.0, 10.0], [20.0, 11.0]]);
        apply(&mut image, &s, |p| p, 1.0);
        let p = image.get_pixel(19, 10).0;
        assert!(p[2] > 20 && p[0] < 250, "edge is softened: {p:?}");
    }

    #[test]
    fn push_moves_pixels() {
        // spec: SMEAR-002
        let mut image = halves();
        let s = stroke(SmearKind::Push, vec![[16.0, 10.0], [24.0, 10.0]]);
        apply(&mut image, &s, |p| p, 1.0);
        let p = image.get_pixel(21, 10).0;
        assert!(p[0] > 100, "red pushed right: {p:?}");
    }

    #[test]
    fn mapping_scales_the_brush() {
        // spec: SMEAR-005
        let mut full = halves();
        let s = stroke(SmearKind::Smudge, vec![[10.0, 10.0], [30.0, 10.0]]);
        apply(&mut full, &s, |p| p, 1.0);
        let mut half = image::imageops::resize(&halves(), 20, 10, image::imageops::Nearest);
        apply(&mut half, &s, |p| [p[0] / 2.0, p[1] / 2.0], 0.5);
        assert!(half.get_pixel(15, 5).0[0] > 60);
    }
}
