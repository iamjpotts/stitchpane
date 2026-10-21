//! Rendering a project's state to pixels (BLEND-004, CANVAS-004, PROJ-005).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use image::RgbaImage;
use image::imageops::FilterType;

use crate::codec;
use crate::geometry::{Point, Rect};
use crate::metadata::Exif;
use crate::model::{Filter, Item, ItemId, ProjectState, SeamGeometry, TransitionKind};

/// A decoded source image.
#[derive(Debug)]
pub struct LoadedSource {
    pub pixels: RgbaImage,
    pub exif: Option<Exif>,
}

/// Why a source could not be loaded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceProblem {
    Missing,
    Unreadable(String),
}

/// Effects that change one item's transparency (BLEND-004).
enum Effect {
    Seam(SeamGeometry),
    Crossfade { overlap: Rect, ratio: f64 },
}

#[derive(Clone, Copy, PartialEq)]
struct ItemKey {
    crop: crate::geometry::PixelRect,
    width: u32,
    height: u32,
    filter: Filter,
}

/// Renders project states, caching decoded sources and resampled items.
#[derive(Default)]
pub struct Renderer {
    sources: HashMap<PathBuf, Result<Arc<LoadedSource>, SourceProblem>>,
    items: HashMap<(PathBuf, ItemId), (ItemKeyBits, Arc<RgbaImage>)>,
}

type ItemKeyBits = (u32, u32, u32, u32, u32, u32, Filter);

impl ItemKey {
    fn bits(self) -> ItemKeyBits {
        (
            self.crop.left,
            self.crop.top,
            self.crop.width,
            self.crop.height,
            self.width,
            self.height,
            self.filter,
        )
    }
}

fn filter_type(filter: Filter) -> FilterType {
    match filter {
        Filter::Nearest => FilterType::Nearest,
        Filter::Bilinear => FilterType::Triangle,
        Filter::Lanczos3 => FilterType::Lanczos3,
    }
}

impl Renderer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads (or returns the cached) source image at `path`.
    pub fn source(&mut self, path: &Path) -> Result<Arc<LoadedSource>, SourceProblem> {
        if let Some(entry) = self.sources.get(path) {
            return entry.clone();
        }
        let entry = if !path.exists() {
            Err(SourceProblem::Missing)
        } else {
            codec::decode(path)
                .map(|d| {
                    Arc::new(LoadedSource {
                        exif: d.exif.as_deref().and_then(Exif::parse),
                        pixels: d.pixels,
                    })
                })
                .map_err(|e| SourceProblem::Unreadable(e.to_string()))
        };
        self.sources.insert(path.to_path_buf(), entry.clone());
        entry
    }

    /// Forgets a cached source, for example after the file was located (PROJ-005).
    pub fn forget(&mut self, path: &Path) {
        self.sources.remove(path);
        self.items.retain(|(p, _), _| p != path);
    }

    /// Sources of `state` that cannot be loaded.
    pub fn problems(
        &mut self,
        state: &ProjectState,
    ) -> Vec<(crate::model::SourceId, SourceProblem)> {
        let mut problems = Vec::new();
        for item in &state.items {
            let Some(path) = state.sources.get(&item.source) else {
                continue;
            };
            if let Err(problem) = self.source(path)
                && !problems.iter().any(|(id, _)| *id == item.source)
            {
                problems.push((item.source, problem));
            }
        }
        problems
    }

    /// The item's source pixels, cropped and resampled to `width` × `height`.
    fn item_pixels(
        &mut self,
        state: &ProjectState,
        item: &Item,
        width: u32,
        height: u32,
        filter: Filter,
    ) -> Option<Arc<RgbaImage>> {
        let path = state.sources.get(&item.source)?.clone();
        let source = self.source(&path).ok()?;
        let key = ItemKey {
            crop: item.crop,
            width,
            height,
            filter,
        }
        .bits();
        if let Some((cached, pixels)) = self.items.get(&(path.clone(), item.id))
            && *cached == key
        {
            return Some(pixels.clone());
        }
        let crop = item.crop;
        let cropped = image::imageops::crop_imm(
            &source.pixels,
            crop.left,
            crop.top,
            crop.width
                .min(source.pixels.width().saturating_sub(crop.left)),
            crop.height
                .min(source.pixels.height().saturating_sub(crop.top)),
        )
        .to_image();
        let pixels = if cropped.dimensions() == (width, height) {
            cropped
        } else {
            image::imageops::resize(&cropped, width, height, filter_type(filter))
        };
        let pixels = Arc::new(pixels);
        self.items.insert((path, item.id), (key, pixels.clone()));
        Some(pixels)
    }

    /// Renders the whole canvas at full size, for export.
    pub fn render(&mut self, state: &ProjectState) -> RgbaImage {
        let canvas = state.canvas_rect();
        let width = (canvas.width.round() as u32).max(1);
        let height = (canvas.height.round() as u32).max(1);
        self.render_region(state, canvas, width, height, 1.0, false)
    }

    /// Renders the canvas area `region` (canvas coordinates) into a `width` × `height`
    /// image, `scale` output pixels per canvas pixel. Areas without items are
    /// transparent. `preview` trades resampling quality for speed.
    pub fn render_region(
        &mut self,
        state: &ProjectState,
        region: Rect,
        width: u32,
        height: u32,
        scale: f64,
        preview: bool,
    ) -> RgbaImage {
        let mut image = RgbaImage::new(width, height);
        self.composite(state, region, scale, preview, &mut image);
        image
    }

    /// Composites the canvas area `region` over `out`, `scale` output pixels per canvas
    /// pixel.
    pub fn composite(
        &mut self,
        state: &ProjectState,
        region: Rect,
        scale: f64,
        preview: bool,
        out: &mut RgbaImage,
    ) {
        let (width, height) = out.dimensions();
        // Items are resampled to at most their full size; larger zooms sample from that.
        let prepared_scale = scale.min(1.0);

        for (index, item) in state.items.iter().enumerate() {
            let bounds = item.bounds();
            let Some(visible) = bounds.intersection(&region) else {
                continue;
            };
            let effects = effects_on(state, index);
            let scaled_w = ((f64::from(item.width) * prepared_scale).round() as u32).max(1);
            let scaled_h = ((f64::from(item.height) * prepared_scale).round() as u32).max(1);
            let filter = if preview && prepared_scale < 1.0 {
                Filter::Bilinear
            } else {
                item.filter
            };
            let pixels = self.item_pixels(state, item, scaled_w, scaled_h, filter);
            let exact = item.quarter_turns().is_some();
            let x0 = ((visible.x - region.x) * scale).floor().max(0.0) as u32;
            let y0 = ((visible.y - region.y) * scale).floor().max(0.0) as u32;
            let x1 = (((visible.right() - region.x) * scale).ceil() as u32).min(width);
            let y1 = (((visible.bottom() - region.y) * scale).ceil() as u32).min(height);
            let sx = f64::from(scaled_w) / f64::from(item.width);
            let sy = f64::from(scaled_h) / f64::from(item.height);
            // Canvas → item-local mapping, computed once per item (see Item::to_local).
            let frame = item.frame();
            let center = frame.center();
            let (sin, cos) = (-item.rotation).to_radians().sin_cos();
            let to_local = |p: Point| {
                let (dx, dy) = (p.x - center.x, p.y - center.y);
                Point::new(
                    center.x + dx * cos - dy * sin - frame.x,
                    center.y + dx * sin + dy * cos - frame.y,
                )
            };
            for py in y0..y1 {
                for px in x0..x1 {
                    let p = Point::new(
                        region.x + (f64::from(px) + 0.5) / scale,
                        region.y + (f64::from(py) + 0.5) / scale,
                    );
                    let local = to_local(p);
                    let (lx, ly) = (local.x * sx, local.y * sy);
                    let color = match &pixels {
                        Some(pixels) => {
                            if exact {
                                sample_nearest(pixels, lx, ly)
                            } else {
                                sample_bilinear(pixels, lx, ly)
                            }
                        }
                        None => placeholder(
                            local.x,
                            local.y,
                            f64::from(item.width),
                            f64::from(item.height),
                        ),
                    };
                    let Some(mut color) = color else { continue };
                    color[3] *= opacity(item, &effects, p) as f32;
                    if color[3] <= 0.0 {
                        continue;
                    }
                    let dst = out.get_pixel_mut(px, py);
                    let below = dst.0.map(|c| f32::from(c) / 255.0);
                    let mixed = over(color, below);
                    dst.0 = mixed.map(to_u8);
                }
            }
        }
    }
}

fn to_u8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Straight-alpha source-over compositing.
fn over(src: [f32; 4], dst: [f32; 4]) -> [f32; 4] {
    let a = src[3] + dst[3] * (1.0 - src[3]);
    if a <= 0.0 {
        return [0.0; 4];
    }
    let mix = |s: f32, d: f32| (s * src[3] + d * dst[3] * (1.0 - src[3])) / a;
    [
        mix(src[0], dst[0]),
        mix(src[1], dst[1]),
        mix(src[2], dst[2]),
        a,
    ]
}

fn texel(image: &RgbaImage, x: u32, y: u32) -> [f32; 4] {
    let p = image.get_pixel(x, y).0;
    [
        f32::from(p[0]) / 255.0,
        f32::from(p[1]) / 255.0,
        f32::from(p[2]) / 255.0,
        f32::from(p[3]) / 255.0,
    ]
}

fn sample_nearest(image: &RgbaImage, x: f64, y: f64) -> Option<[f32; 4]> {
    let (w, h) = image.dimensions();
    if x < 0.0 || y < 0.0 || x >= f64::from(w) || y >= f64::from(h) {
        return None;
    }
    Some(texel(image, x as u32, y as u32))
}

/// Bilinear sampling; pixels at the item's edge fade with partial coverage.
fn sample_bilinear(image: &RgbaImage, x: f64, y: f64) -> Option<[f32; 4]> {
    let (w, h) = image.dimensions();
    let (fw, fh) = (f64::from(w), f64::from(h));
    if x < -0.5 || y < -0.5 || x >= fw + 0.5 || y >= fh + 0.5 {
        return None;
    }
    let gx = x - 0.5;
    let gy = y - 0.5;
    let x0 = gx.floor();
    let y0 = gy.floor();
    let tx = (gx - x0) as f32;
    let ty = (gy - y0) as f32;
    let mut acc = [0f32; 4];
    for (dx, dy, weight) in [
        (0.0, 0.0, (1.0 - tx) * (1.0 - ty)),
        (1.0, 0.0, tx * (1.0 - ty)),
        (0.0, 1.0, (1.0 - tx) * ty),
        (1.0, 1.0, tx * ty),
    ] {
        let sx = x0 + dx;
        let sy = y0 + dy;
        if sx < 0.0 || sy < 0.0 || sx >= fw || sy >= fh || weight == 0.0 {
            continue;
        }
        let t = texel(image, sx as u32, sy as u32);
        // Premultiply so transparent texels do not darken edges.
        acc[0] += t[0] * t[3] * weight;
        acc[1] += t[1] * t[3] * weight;
        acc[2] += t[2] * t[3] * weight;
        acc[3] += t[3] * weight;
    }
    if acc[3] <= 0.0 {
        return None;
    }
    Some([acc[0] / acc[3], acc[1] / acc[3], acc[2] / acc[3], acc[3]])
}

/// Gray hatching drawn for items whose source is missing (PROJ-005). The UI adds the label.
fn placeholder(x: f64, y: f64, w: f64, h: f64) -> Option<[f32; 4]> {
    if x < 0.0 || y < 0.0 || x >= w || y >= h {
        return None;
    }
    let stripe = ((x + y) / 12.0).floor() as i64 % 2 == 0;
    let v = if stripe { 0.55 } else { 0.65 };
    Some([v, v, v, 1.0])
}

fn effects_on(state: &ProjectState, index: usize) -> Vec<Effect> {
    let item = &state.items[index];
    state
        .transitions_of(item.id)
        .filter_map(|t| {
            let geometry = state.transition_geometry(t)?;
            if geometry.upper != item.id {
                return None;
            }
            match t.kind {
                TransitionKind::GradientSeam { .. } => geometry.seam.map(Effect::Seam),
                TransitionKind::Crossfade { ratio } => Some(Effect::Crossfade {
                    overlap: geometry.overlap,
                    ratio,
                }),
            }
        })
        .collect()
}

/// Opacity multiplier of `item` at canvas point `p`: feathered edges and transitions
/// multiply (BLEND-004).
fn opacity(item: &Item, effects: &[Effect], p: Point) -> f64 {
    let mut alpha = feather_opacity(item, p);
    for effect in effects {
        alpha *= match effect {
            Effect::Seam(seam) => seam.opacity(p),
            Effect::Crossfade { overlap, ratio } => {
                if overlap.contains(p) {
                    *ratio
                } else {
                    1.0
                }
            }
        };
    }
    alpha
}

/// Feathered edges are bands on the canvas along the item's unrotated frame, so rotating
/// the item does not move them (BLEND-007).
pub fn feather_opacity(item: &Item, p: Point) -> f64 {
    let Some(f) = item.feather else { return 1.0 };
    let r = item.frame();
    let ramp = |distance: f64| f.curve.apply(distance / f.width);
    let mut alpha = 1.0;
    if f.left {
        alpha *= ramp(p.x - r.x);
    }
    if f.right {
        alpha *= ramp(r.right() - p.x);
    }
    if f.top {
        alpha *= ramp(p.y - r.y);
    }
    if f.bottom {
        alpha *= ramp(r.bottom() - p.y);
    }
    alpha
}
