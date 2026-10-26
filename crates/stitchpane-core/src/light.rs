//! Light adjustments (LIGHT-002 to LIGHT-005), and the conversions between encoded
//! (sRGB) values and linear light that they and dodge and burn share.

use std::sync::OnceLock;

use image::RgbaImage;

use crate::model::Light;
use crate::panic_free::{self, Clamped};

/// Stops added at 100% shadows to the darkest areas, and removed at 100% highlights from
/// the brightest (LIGHT-003).
const SHADOWS_STOPS: f32 = 2.5;
const HIGHLIGHTS_STOPS: f32 = 1.5;

/// Brightness, `0..=1` encoded, below which shadows lighten and above which highlights
/// darken (LIGHT-003).
const MIDDLE: f32 = 0.5;

/// How strongly an edge stops the shadows and highlights blur (guided filter epsilon,
/// in squared encoded brightness).
const EDGE: f32 = 0.01;

/// Entries in the linear-to-encoded table.
const ENCODE_STEPS: usize = 16384;

fn decode_table() -> &'static [f32; 256] {
    static TABLE: OnceLock<[f32; 256]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = [0f32; 256];
        for (i, v) in table.iter_mut().enumerate() {
            let c = i as f64 / 255.0;
            *v = if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            } as f32;
        }
        table
    })
}

fn encode_table() -> &'static [f32] {
    static TABLE: OnceLock<Vec<f32>> = OnceLock::new();
    TABLE.get_or_init(|| {
        (0..=ENCODE_STEPS)
            .map(|i| {
                let l = i as f64 / ENCODE_STEPS as f64;
                (if l <= 0.003_130_8 {
                    l * 12.92
                } else {
                    1.055 * l.powf(1.0 / 2.4) - 0.055
                }) as f32
            })
            .collect()
    })
}

/// Linear light of an encoded value.
pub(crate) fn to_linear(v: u8) -> f32 {
    decode_table().get(usize::from(v)).copied().unwrap_or(0.0)
}

/// Encoded value, `0..=1`, of linear light (clipped to `0..=1`).
pub(crate) fn to_encoded(l: f32) -> f32 {
    let index = (l.clamped(0.0, 1.0) * ENCODE_STEPS as f32).round() as usize;
    encode_table().get(index).copied().unwrap_or(1.0)
}

/// Relative luminance of linear red, green and blue.
pub(crate) fn luminance(rgb: [f32; 3]) -> f32 {
    0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]
}

/// An 8-bit channel value of linear light.
pub(crate) fn to_u8(l: f32) -> u8 {
    (to_encoded(l) * 255.0).round() as u8
}

/// Applies `light` to `image`, which has `scale` pixels per source pixel (LIGHT-005).
pub fn apply(image: &mut RgbaImage, light: &Light, scale: f64) {
    if light.is_neutral() {
        return;
    }
    let (width, height) = (image.width() as usize, image.height() as usize);
    let mut pixels: Vec<[f32; 3]> = image
        .pixels()
        .map(|p| [to_linear(p.0[0]), to_linear(p.0[1]), to_linear(p.0[2])])
        .collect();

    // LIGHT-002: each stop doubles the light.
    if light.exposure != 0.0 {
        let gain = 2f32.powf(light.exposure as f32);
        for p in &mut pixels {
            *p = p.map(|c| c * gain);
        }
    }

    // LIGHT-003: lighten or darken by the brightness of each pixel's surroundings.
    if light.shadows > 0.0 || light.highlights > 0.0 {
        let brightness: Vec<f32> = pixels.iter().map(|p| to_encoded(luminance(*p))).collect();
        let half = ((light.radius * scale / 2.0).round().max(1.0)) as usize;
        let area = guided_filter(&brightness, width, height, half, EDGE);
        let (shadows, highlights) = (light.shadows as f32, light.highlights as f32);
        for (p, &around) in pixels.iter_mut().zip(&area) {
            let dark = smoothstep((MIDDLE - around) / MIDDLE);
            let bright = smoothstep((around - MIDDLE) / (1.0 - MIDDLE));
            let stops = shadows * SHADOWS_STOPS * dark - highlights * HIGHLIGHTS_STOPS * bright;
            if stops != 0.0 {
                let gain = 2f32.powf(stops);
                *p = p.map(|c| c * gain);
            }
        }
    }

    // LIGHT-004: levels on the encoded values, read exactly from the image when nothing
    // above changed it.
    let unchanged = light.exposure == 0.0 && light.shadows == 0.0 && light.highlights == 0.0;
    let black = light.black as f32 / 255.0;
    let range = ((light.white - light.black) as f32 / 255.0).max(1.0 / 255.0);
    let power = 1.0 / light.midtones as f32;
    let levels = |v: f32| -> u8 {
        let v = ((v - black) / range).clamped(0.0, 1.0).powf(power);
        (v * 255.0).round() as u8
    };
    for (out, p) in image.pixels_mut().zip(&pixels) {
        for (channel, &l) in out.0.iter_mut().zip(p) {
            let v = if unchanged {
                f32::from(*channel) / 255.0
            } else {
                to_encoded(l)
            };
            *channel = levels(v);
        }
    }
}

/// `0` at or below 0, `1` at or above 1, smooth between.
fn smoothstep(t: f32) -> f32 {
    let t = t.clamped(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Edge-preserving blur of `values` (a `width` × `height` plane) guided by itself: a
/// guided filter over squares `2 * half + 1` across. `edge` is the variance above which
/// an edge is kept.
fn guided_filter(values: &[f32], width: usize, height: usize, half: usize, edge: f32) -> Vec<f32> {
    let mean = box_mean(values, width, height, half);
    let squares: Vec<f32> = values.iter().map(|v| v * v).collect();
    let mean_sq = box_mean(&squares, width, height, half);
    let (a, b): (Vec<f32>, Vec<f32>) = mean
        .iter()
        .zip(&mean_sq)
        .map(|(&m, &sq)| {
            let variance = (sq - m * m).max(0.0);
            let a = variance / (variance + edge);
            (a, m - a * m)
        })
        .unzip();
    let mean_a = box_mean(&a, width, height, half);
    let mean_b = box_mean(&b, width, height, half);
    values
        .iter()
        .zip(mean_a.iter().zip(&mean_b))
        .map(|(&v, (&a, &b))| a * v + b)
        .collect()
}

/// Mean of each square `2 * half + 1` across, of the parts inside the plane.
fn box_mean(values: &[f32], width: usize, height: usize, half: usize) -> Vec<f32> {
    let rows = mean_rows(values, width, half);
    let columns = mean_rows(&transpose(&rows, width, height), height, half);
    transpose(&columns, height, width)
}

/// Mean along each row of `width` values, over `2 * half + 1` values.
fn mean_rows(values: &[f32], width: usize, half: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(values.len());
    let mut sums = Vec::with_capacity(width.saturating_add(1));
    for row in panic_free::chunks(values, width) {
        sums.clear();
        let mut sum = 0f64;
        sums.push(sum);
        for &v in row {
            sum += f64::from(v);
            sums.push(sum);
        }
        let len = row.len();
        for x in 0..len {
            let low = x.saturating_sub(half);
            let high = x.saturating_add(half).saturating_add(1).min(len);
            let total =
                sums.get(high).copied().unwrap_or(0.0) - sums.get(low).copied().unwrap_or(0.0);
            let count = high.saturating_sub(low).max(1);
            out.push((total / count as f64) as f32);
        }
    }
    out
}

/// The plane `width` × `height` with rows and columns swapped.
fn transpose(values: &[f32], width: usize, height: usize) -> Vec<f32> {
    let mut out = vec![0f32; values.len()];
    for (y, row) in panic_free::chunks(values, width).enumerate() {
        for (x, &v) in row.iter().enumerate() {
            if let Some(slot) = out.get_mut(x.saturating_mul(height).saturating_add(y)) {
                *slot = v;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gray(v: u8) -> RgbaImage {
        RgbaImage::from_pixel(40, 20, image::Rgba([v, v, v, 255]))
    }

    #[test]
    fn neutral_changes_nothing() {
        // spec: LIGHT-002, LIGHT-003, LIGHT-004
        let mut image = RgbaImage::from_fn(30, 30, |x, y| {
            image::Rgba([(x * 8) as u8, (y * 8) as u8, 100, 200])
        });
        let before = image.clone();
        apply(&mut image, &Light::neutral(10.0), 1.0);
        assert_eq!(image, before);
    }

    #[test]
    fn round_trip_keeps_values() {
        // spec: LIGHT-005
        for v in 0..=255u8 {
            assert_eq!(to_u8(to_linear(v)), v);
        }
    }

    #[test]
    fn exposure_doubles_the_light() {
        // spec: LIGHT-002
        let mut image = gray(100);
        apply(
            &mut image,
            &Light {
                exposure: 1.0,
                ..Light::neutral(10.0)
            },
            1.0,
        );
        let expected = to_u8(to_linear(100) * 2.0);
        assert_eq!(image.get_pixel(5, 5).0, [expected, expected, expected, 255]);
    }

    /// Dark on the left half, bright on the right.
    fn halves(dark: u8, bright: u8) -> RgbaImage {
        RgbaImage::from_fn(120, 40, |x, _| {
            let v = if x < 60 { dark } else { bright };
            image::Rgba([v, v, v, 255])
        })
    }

    #[test]
    fn shadows_lighten_dark_areas_up_to_the_edge() {
        // spec: LIGHT-003
        let mut image = halves(30, 220);
        let light = Light {
            shadows: 1.0,
            ..Light::neutral(60.0)
        };
        apply(&mut image, &light, 1.0);
        let far = image.get_pixel(5, 20).0[0];
        let near = image.get_pixel(58, 20).0[0];
        assert!(far > 60, "the dark half is lighter: {far}");
        // no halo: the dark side of the edge is lifted nearly as much as the rest of the
        // dark half (a plain blur would hardly lift it)
        let (lift_far, lift_near) = (f32::from(far - 30), f32::from(near.saturating_sub(30)));
        assert!(
            lift_near >= 0.8 * lift_far,
            "far {far}, near the edge {near}"
        );
        assert_eq!(
            image.get_pixel(100, 20).0[0],
            220,
            "the bright half is unchanged"
        );
    }

    #[test]
    fn highlights_darken_bright_areas_up_to_the_edge() {
        // spec: LIGHT-003
        let mut image = halves(30, 240);
        let light = Light {
            highlights: 1.0,
            ..Light::neutral(60.0)
        };
        apply(&mut image, &light, 1.0);
        let far = image.get_pixel(115, 20).0[0];
        let near = image.get_pixel(61, 20).0[0];
        assert!(far < 220, "the bright half is darker: {far}");
        let (drop_far, drop_near) = (f32::from(240 - far), f32::from(240u8.saturating_sub(near)));
        assert!(
            drop_near >= 0.8 * drop_far,
            "far {far}, near the edge {near}"
        );
        assert_eq!(
            image.get_pixel(10, 20).0[0],
            30,
            "the dark half is unchanged"
        );
    }

    #[test]
    fn exposure_and_shadows_keep_the_hue() {
        // spec: LIGHT-005
        let color = [90u8, 50, 20];
        let ratio = |p: [u8; 4]| to_linear(p[0]) / to_linear(p[1]);
        let mut image =
            RgbaImage::from_pixel(40, 40, image::Rgba([color[0], color[1], color[2], 255]));
        let before = ratio(image.get_pixel(0, 0).0);
        let light = Light {
            exposure: 0.5,
            shadows: 0.5,
            ..Light::neutral(20.0)
        };
        apply(&mut image, &light, 1.0);
        let after = image.get_pixel(20, 20).0;
        assert!(after[1] > color[1], "lighter: {after:?}");
        assert!(
            (ratio(after) - before).abs() < 0.05,
            "same hue: {before} vs {}",
            ratio(after)
        );
    }

    #[test]
    fn levels_map_black_white_and_midtones() {
        // spec: LIGHT-004
        let image = RgbaImage::from_fn(3, 1, |x, _| {
            let v = [20u8, 135, 250][x as usize];
            image::Rgba([v, v, v, 255])
        });
        let mut out = image.clone();
        let light = Light {
            black: 20.0,
            white: 250.0,
            midtones: 2.0,
            ..Light::neutral(10.0)
        };
        apply(&mut out, &light, 1.0);
        assert_eq!(out.get_pixel(0, 0).0[0], 0);
        assert_eq!(out.get_pixel(2, 0).0[0], 255);
        // halfway between black and white, lightened by midtones 2: sqrt(0.5)
        assert_eq!(
            out.get_pixel(1, 0).0[0],
            (0.5f32.sqrt() * 255.0).round() as u8
        );
    }
}
