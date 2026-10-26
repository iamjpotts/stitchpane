//! Brush strokes applied to pixels: smear (SMEAR-002 to SMEAR-005) and dodge and burn
//! (DODGE-002).

use std::hash::{DefaultHasher, Hash, Hasher};

use image::RgbaImage;

use crate::light;
use crate::model::{Stroke, StrokeKind, ToneRange};
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
        StrokeKind::Smudge => radius / 12.0,
        StrokeKind::Blur | StrokeKind::Push | StrokeKind::Dodge | StrokeKind::Burn => radius / 4.0,
    };
    let dabs = dabs(stroke.points.iter().map(|&p| map(p)), spacing.max(0.5));
    let strength = stroke.strength as f32;
    match stroke.kind {
        StrokeKind::Smudge => smudge(image, &dabs, radius, strength),
        StrokeKind::Blur => blur(image, &dabs, radius, strength),
        StrokeKind::Push => push(image, &dabs, radius, strength),
        StrokeKind::Dodge | StrokeKind::Burn => {
            let stops = if stroke.kind == StrokeKind::Dodge {
                strength
            } else {
                -strength
            };
            tone(
                image,
                &dabs,
                radius,
                stops,
                stroke.range.unwrap_or_default(),
            );
        }
    }
}

/// A hash of strokes, to tell apart pixels made with different strokes.
pub fn strokes_hash<'a>(strokes: impl IntoIterator<Item = &'a Stroke>) -> u64 {
    let mut hasher = DefaultHasher::new();
    for stroke in strokes {
        stroke.kind.hash(&mut hasher);
        stroke.size.to_bits().hash(&mut hasher);
        stroke.strength.to_bits().hash(&mut hasher);
        stroke.range.hash(&mut hasher);
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
    let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) else {
        return None;
    };
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

/// How much dodge and burn changes a pixel of encoded brightness `v`, `0..=1`, for a
/// range (DODGE-002).
fn range_weight(range: ToneRange, v: f32) -> f32 {
    let smooth = |t: f32| {
        let t = t.clamped(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    match range {
        ToneRange::Shadows => smooth((0.6 - v) / 0.6),
        ToneRange::Midtones => 1.0 - (2.0 * v - 1.0).powi(2),
        ToneRange::Highlights => smooth((v - 0.4) / 0.6),
    }
}

/// Lightens (positive `stops`) or darkens the pixels under the brush, by up to `stops`
/// at the brush center. The stroke's coverage is the most any dab gives a pixel, so a
/// stroke does not build up where it crosses itself (DODGE-002).
fn tone(image: &mut RgbaImage, dabs: &[[f64; 2]], radius: f64, stops: f32, range: ToneRange) {
    let (w, h) = (i64::from(image.width()), i64::from(image.height()));
    let r = radius.ceil() as i64;
    let low = |v: f64| (v.floor() as i64).saturating_sub(r).max(0);
    let high = |v: f64, limit: i64| {
        (v.floor() as i64)
            .saturating_add(r)
            .saturating_add(1)
            .min(limit)
    };
    let x0 = dabs.iter().map(|d| low(d[0])).min().unwrap_or(0);
    let y0 = dabs.iter().map(|d| low(d[1])).min().unwrap_or(0);
    let x1 = dabs.iter().map(|d| high(d[0], w)).max().unwrap_or(0);
    let y1 = dabs.iter().map(|d| high(d[1], h)).max().unwrap_or(0);
    let side = x1.saturating_sub(x0).max(0) as usize;
    let rows = y1.saturating_sub(y0).max(0) as usize;
    let mut coverage = vec![0f32; side.saturating_mul(rows)];
    for &dab in dabs {
        for (x, y, _, weight) in brush_pixels(image, dab, radius) {
            let i = (y.saturating_sub(y0) as usize)
                .saturating_mul(side)
                .saturating_add(x.saturating_sub(x0) as usize);
            if let Some(c) = coverage.get_mut(i) {
                *c = c.max(weight);
            }
        }
    }
    for (j, row) in panic_free::chunks(&coverage, side).enumerate() {
        for (i, &c) in row.iter().enumerate() {
            if c <= 0.0 {
                continue;
            }
            let (x, y) = (x0.saturating_add(i as i64), y0.saturating_add(j as i64));
            let (Ok(px), Ok(py)) = (u32::try_from(x), u32::try_from(y)) else {
                continue;
            };
            let Some(pixel) = image.get_pixel_mut_checked(px, py) else {
                continue;
            };
            let rgb = [pixel.0[0], pixel.0[1], pixel.0[2]].map(light::to_linear);
            let v = light::to_encoded(light::luminance(rgb));
            let gain = 2f32.powf(stops * c * range_weight(range, v));
            for (out, l) in pixel.0.iter_mut().zip(rgb) {
                *out = light::to_u8(l * gain);
            }
        }
    }
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

    fn stroke(kind: StrokeKind, points: Vec<[f64; 2]>) -> Stroke {
        Stroke {
            kind,
            size: 10.0,
            strength: 0.8,
            points,
            range: None,
        }
    }

    #[test]
    fn smudge_drags_color_along_the_stroke() {
        // spec: SMEAR-002
        let mut image = halves();
        let s = stroke(StrokeKind::Smudge, vec![[10.0, 10.0], [30.0, 10.0]]);
        apply(&mut image, &s, |p| p, 1.0);
        let p = image.get_pixel(30, 10).0;
        assert!(p[0] > 60, "red was dragged into blue: {p:?}");
        assert_eq!(image.get_pixel(30, 0).0, [0, 0, 255, 255]);
    }

    #[test]
    fn blur_mixes_the_edge() {
        // spec: SMEAR-002
        let mut image = halves();
        let s = stroke(StrokeKind::Blur, vec![[20.0, 10.0], [20.0, 11.0]]);
        apply(&mut image, &s, |p| p, 1.0);
        let p = image.get_pixel(19, 10).0;
        assert!(p[2] > 20 && p[0] < 250, "edge is softened: {p:?}");
    }

    #[test]
    fn push_moves_pixels() {
        // spec: SMEAR-002
        let mut image = halves();
        let s = stroke(StrokeKind::Push, vec![[16.0, 10.0], [24.0, 10.0]]);
        apply(&mut image, &s, |p| p, 1.0);
        let p = image.get_pixel(21, 10).0;
        assert!(p[0] > 100, "red pushed right: {p:?}");
    }

    fn tone_stroke(kind: StrokeKind, range: ToneRange, points: Vec<[f64; 2]>) -> Stroke {
        Stroke {
            kind,
            size: 20.0,
            strength: 1.0,
            points,
            range: Some(range),
        }
    }

    /// Dark on the left, middle gray in the center, bright on the right.
    fn steps() -> RgbaImage {
        RgbaImage::from_fn(90, 30, |x, _| {
            let v = [40u8, 128, 220][(x / 30) as usize];
            image::Rgba([v, v, v, 255])
        })
    }

    #[test]
    fn dodge_lightens_and_burn_darkens_under_the_brush() {
        // spec: DODGE-002
        for (kind, lighter) in [(StrokeKind::Dodge, true), (StrokeKind::Burn, false)] {
            let mut image = steps();
            let s = tone_stroke(kind, ToneRange::Midtones, vec![[45.0, 15.0]]);
            apply(&mut image, &s, |p| p, 1.0);
            let center = image.get_pixel(45, 15).0[0];
            // one stop at the brush center at 100%, in linear light
            let expected = light::to_u8(light::to_linear(128) * if lighter { 2.0 } else { 0.5 });
            assert!(
                center.abs_diff(expected) <= 2,
                "{kind:?}: {center}, expected {expected}"
            );
            assert_eq!(image.get_pixel(45, 0).0[0], 128, "outside the brush");
        }
    }

    #[test]
    fn the_range_limits_dodge_to_its_pixels() {
        // spec: DODGE-002
        let mut image = steps();
        let path = vec![[5.0, 15.0], [85.0, 15.0]];
        apply(
            &mut image,
            &tone_stroke(StrokeKind::Dodge, ToneRange::Highlights, path),
            |p| p,
            1.0,
        );
        assert_eq!(image.get_pixel(15, 15).0[0], 40, "shadows are left alone");
        assert!(image.get_pixel(75, 15).0[0] > 230, "highlights are lighter");
    }

    #[test]
    fn a_stroke_does_not_build_up_where_it_crosses_itself() {
        // spec: DODGE-002
        let once = tone_stroke(StrokeKind::Dodge, ToneRange::Midtones, vec![[45.0, 15.0]]);
        let back_and_forth = tone_stroke(
            StrokeKind::Dodge,
            ToneRange::Midtones,
            vec![
                [45.0, 15.0],
                [50.0, 15.0],
                [45.0, 15.0],
                [50.0, 15.0],
                [45.0, 15.0],
            ],
        );
        let (mut a, mut b) = (steps(), steps());
        apply(&mut a, &once, |p| p, 1.0);
        apply(&mut b, &back_and_forth, |p| p, 1.0);
        assert_eq!(a.get_pixel(45, 15), b.get_pixel(45, 15));
    }

    #[test]
    fn mapping_scales_the_brush() {
        // spec: SMEAR-005
        let mut full = halves();
        let s = stroke(StrokeKind::Smudge, vec![[10.0, 10.0], [30.0, 10.0]]);
        apply(&mut full, &s, |p| p, 1.0);
        let mut half = image::imageops::resize(&halves(), 20, 10, image::imageops::Nearest);
        apply(&mut half, &s, |p| [p[0] / 2.0, p[1] / 2.0], 0.5);
        assert!(half.get_pixel(15, 5).0[0] > 60);
    }
}
