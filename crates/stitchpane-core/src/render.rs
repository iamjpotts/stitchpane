//! Rendering a project's state to pixels (BLEND-004, CANVAS-004, PROJ-005).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc;

use image::RgbaImage;
use image::imageops::FilterType;

use crate::codec;
use crate::geometry::{Point, Rect};
use crate::light;
use crate::metadata::Exif;
use crate::model::{
    Filter, Item, ItemId, Light, ProjectState, SeamGeometry, Stroke, TransitionKind,
};
use crate::panic_free::{self, Clamped};
use crate::retouch;

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
    /// Hash of the item's brush strokes (SMEAR-004, DODGE-004).
    strokes: u64,
    /// Hash of the item's light settings (LIGHT-006).
    light: u64,
}

/// Renders project states, caching decoded sources and resampled items.
#[derive(Default)]
pub struct Renderer {
    sources: HashMap<PathBuf, Result<Arc<LoadedSource>, SourceProblem>>,
    items: HashMap<ItemCacheKey, (ItemKeyBits, Arc<RgbaImage>)>,
    /// Sources with an item's smear strokes applied, by stroke hash.
    smeared: HashMap<ItemCacheKey, (u64, Arc<RgbaImage>)>,
    worker: Option<Worker>,
    /// A stroke being drawn, shown on previews only (SMEAR-004).
    live: Option<(ItemId, Stroke)>,
    /// The last preview drawn with the live stroke, shown after the stroke is recorded
    /// until its full-quality resample is ready.
    last_live: Option<(ItemCacheKey, ItemKeyBits, Arc<RgbaImage>)>,
}

type ItemKeyBits = (u32, u32, u32, u32, u32, u32, Filter, u64, u64);
type ItemCacheKey = (PathBuf, ItemId);

/// A resample to do in the background (UI-009).
struct Job {
    key: ItemCacheKey,
    bits: ItemKeyBits,
    source: Arc<LoadedSource>,
    strokes: Vec<Stroke>,
    light: Option<Light>,
}

struct Done {
    key: ItemCacheKey,
    bits: ItemKeyBits,
    pixels: Arc<RgbaImage>,
}

/// A thread that resamples items for previews, so drawing never waits for it (UI-009).
struct Worker {
    jobs: mpsc::Sender<Job>,
    done: mpsc::Receiver<Done>,
    /// The latest size requested for each item and not yet received.
    pending: HashMap<ItemCacheKey, ItemKeyBits>,
}

impl Worker {
    /// `None` when the thread cannot be created.
    fn start() -> Option<Self> {
        let (jobs, job_rx) = mpsc::channel::<Job>();
        let (done_tx, done) = mpsc::channel();
        let spawned = std::thread::Builder::new().spawn(move || {
            while let Ok(first) = job_rx.recv() {
                // Only the latest request for each item matters.
                let mut latest: Vec<Job> = vec![first];
                while let Ok(job) = job_rx.try_recv() {
                    latest.retain(|j| j.key != job.key);
                    latest.push(job);
                }
                for job in latest {
                    let pixels = if job.strokes.is_empty() {
                        resample(&job.source.pixels, job.bits, job.light)
                    } else {
                        let smeared = smear(&job.source.pixels, &job.strokes);
                        resample(&smeared, job.bits, job.light)
                    };
                    let pixels = Arc::new(pixels);
                    let done = Done {
                        key: job.key,
                        bits: job.bits,
                        pixels,
                    };
                    // the receiver is gone, so nobody wants the results
                    if let Err(_) = done_tx.send(done) {
                        return;
                    }
                }
            }
        });
        let Ok(_) = spawned else { return None };
        Some(Self {
            jobs,
            done,
            pending: HashMap::new(),
        })
    }
}

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
            self.strokes,
            self.light,
        )
    }
}

/// A hash of light settings; settings that change nothing hash as none (LIGHT-006).
fn light_hash(light: Option<&Light>) -> u64 {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let Some(light) = light.filter(|l| !l.is_neutral()) else {
        return 0;
    };
    let mut hasher = DefaultHasher::new();
    for v in [
        light.exposure,
        light.shadows,
        light.highlights,
        light.radius,
        light.black,
        light.white,
        light.midtones,
    ] {
        v.to_bits().hash(&mut hasher);
    }
    hasher.finish()
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
        self.smeared.retain(|(p, _), _| p != path);
        if let Some(worker) = &mut self.worker {
            worker.pending.retain(|(p, _), _| p != path);
        }
    }

    /// Resamples items for previews in a background thread (UI-009). Until a resample is
    /// ready, previews stretch the item's previous one. If the thread cannot be created,
    /// previews stay resampled in the foreground.
    pub fn set_background(&mut self, enabled: bool) {
        if enabled != self.worker.is_some() {
            self.worker = if enabled { Worker::start() } else { None };
        }
    }

    /// Whether previews are resampled in the background.
    pub fn background(&self) -> bool {
        self.worker.is_some()
    }

    /// Sets the stroke being drawn, shown on previews until it is recorded (SMEAR-004).
    pub fn set_live_stroke(&mut self, live: Option<(ItemId, Stroke)>) {
        self.live = live;
    }

    /// Whether background resamples are still being made.
    pub fn busy(&self) -> bool {
        self.worker.as_ref().is_some_and(|w| !w.pending.is_empty())
    }

    /// Takes finished background resamples. Returns true if any arrived, meaning
    /// previews may have changed.
    pub fn poll(&mut self) -> bool {
        let Some(worker) = &mut self.worker else {
            return false;
        };
        let mut arrived = false;
        while let Ok(done) = worker.done.try_recv() {
            // Ignore results for sources forgotten since they were requested.
            let Some(latest) = worker.pending.get(&done.key) else {
                continue;
            };
            if *latest == done.bits {
                worker.pending.remove(&done.key);
                if self.last_live.as_ref().is_some_and(|l| l.0 == done.key) {
                    self.last_live = None;
                }
            }
            self.items.insert(done.key, (done.bits, done.pixels));
            arrived = true;
        }
        arrived
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

    /// The item's source pixels, cropped and resampled to `width` × `height`. With
    /// `stale_ok` and a background worker, a missing resample is requested from the
    /// worker and a previous one of a different size is returned meanwhile.
    fn item_pixels(
        &mut self,
        state: &ProjectState,
        item: &Item,
        width: u32,
        height: u32,
        filter: Filter,
        stale_ok: bool,
    ) -> Option<Arc<RgbaImage>> {
        let path = state.sources.get(&item.source)?.clone();
        let Ok(source) = self.source(&path) else {
            return None;
        };
        let strokes = state.strokes_of(item.id);
        let key = ItemKey {
            crop: item.crop,
            width,
            height,
            filter,
            strokes: retouch::strokes_hash(strokes),
            light: light_hash(item.light.as_ref()),
        }
        .bits();
        let cache_key = (path, item.id);
        let pixels = self.item_pixels_for(&cache_key, key, source, strokes, item.light, stale_ok);
        match &self.live {
            Some((id, stroke)) if stale_ok && *id == item.id => {
                // Draw the live stroke at preview resolution (SMEAR-004).
                let mut live = (*pixels).clone();
                let crop = item.crop;
                let sx = f64::from(live.width()) / f64::from(crop.width);
                let sy = f64::from(live.height()) / f64::from(crop.height);
                let map = |p: [f64; 2]| {
                    [
                        (p[0] - f64::from(crop.left)) * sx,
                        (p[1] - f64::from(crop.top)) * sy,
                    ]
                };
                retouch::apply(&mut live, stroke, map, (sx + sy) / 2.0);
                let live = Arc::new(live);
                let mut bits = key;
                bits.7 = retouch::strokes_hash(strokes.iter().chain([stroke]));
                self.last_live = Some((cache_key, bits, live.clone()));
                Some(live)
            }
            _ => Some(pixels),
        }
    }

    /// See [`Renderer::item_pixels`]; `key` includes the strokes and light.
    fn item_pixels_for(
        &mut self,
        cache_key: &ItemCacheKey,
        key: ItemKeyBits,
        source: Arc<LoadedSource>,
        strokes: &[Stroke],
        light: Option<Light>,
        stale_ok: bool,
    ) -> Arc<RgbaImage> {
        let cache_key = cache_key.clone();
        if let Some((cached, pixels)) = self.items.get(&cache_key) {
            let same_crop =
                (cached.0, cached.1, cached.2, cached.3) == (key.0, key.1, key.2, key.3);
            if *cached == key {
                return pixels.clone();
            }
            if stale_ok
                && same_crop
                && let Some(worker) = &mut self.worker
            {
                // A just-recorded stroke looks as it did while drawn.
                let stale = match &self.last_live {
                    Some((k, bits, live)) if *k == cache_key && *bits == key => live.clone(),
                    _ => pixels.clone(),
                };
                if worker.pending.get(&cache_key) != Some(&key) {
                    worker.pending.insert(cache_key.clone(), key);
                    let job = Job {
                        key: cache_key,
                        bits: key,
                        source,
                        strokes: strokes.to_vec(),
                        light,
                    };
                    // the worker thread has stopped; resample in the foreground from now on
                    if let Err(_) = worker.jobs.send(job) {
                        self.worker = None;
                    }
                }
                return stale;
            }
        }
        let pixels = if strokes.is_empty() {
            Arc::new(resample(&source.pixels, key, light))
        } else {
            let smeared = match self.smeared.get(&cache_key) {
                Some((hash, smeared)) if *hash == key.7 => smeared.clone(),
                _ => {
                    let smeared = Arc::new(smear(&source.pixels, strokes));
                    self.smeared
                        .insert(cache_key.clone(), (key.7, smeared.clone()));
                    smeared
                }
            };
            Arc::new(resample(&smeared, key, light))
        };
        if let Some(worker) = &mut self.worker {
            worker.pending.remove(&cache_key);
        }
        self.items.insert(cache_key, (key, pixels.clone()));
        pixels
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
        // an image too large to address renders as an empty one
        let mut image = panic_free::blank_image(width, height).unwrap_or_default();
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

        let mut layers = Vec::new();
        for (index, item) in state.items.iter().enumerate() {
            let bounds = item.bounds();
            let Some(visible) = bounds.intersection(&region) else {
                continue;
            };
            let scaled_w = ((f64::from(item.width) * prepared_scale).round() as u32).max(1);
            let scaled_h = ((f64::from(item.height) * prepared_scale).round() as u32).max(1);
            let filter = if preview && prepared_scale < 1.0 {
                Filter::Bilinear
            } else {
                item.filter
            };
            let pixels = self.item_pixels(state, item, scaled_w, scaled_h, filter, preview);
            // A stale preview resample has another size and is stretched (UI-009).
            let (pixels_w, pixels_h) = pixels
                .as_ref()
                .map_or((scaled_w, scaled_h), |p| p.dimensions());
            layers.push(Layer {
                item: *item,
                effects: effects_on(state, index),
                exact: item.quarter_turns().is_some()
                    && (pixels_w, pixels_h) == (scaled_w, scaled_h),
                pixels,
                sx: f64::from(pixels_w) / f64::from(item.width),
                sy: f64::from(pixels_h) / f64::from(item.height),
                x0: ((visible.x - region.x) * scale).floor().max(0.0) as u32,
                y0: ((visible.y - region.y) * scale).floor().max(0.0) as u32,
                x1: (((visible.right() - region.x) * scale).ceil() as u32).min(width),
                y1: (((visible.bottom() - region.y) * scale).ceil() as u32).min(height),
            });
        }
        if layers.is_empty() || width == 0 {
            return;
        }

        // Bands of rows are composited in parallel, each through all layers in order.
        let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
        let threads = u32::try_from(threads).unwrap_or(u32::MAX).max(1);
        // rows per band, rounded up
        let band_rows = height
            .saturating_add(threads.saturating_sub(1))
            .checked_div(threads)
            .unwrap_or(height)
            .max(16);
        let band_bytes = (band_rows as usize).saturating_mul((width as usize).saturating_mul(4));
        let draw = |rows: &mut [u8], band: usize| {
            let first_row = (band as u32).saturating_mul(band_rows);
            for layer in &layers {
                layer.draw(rows, first_row, width, region, scale);
            }
        };
        // Bands whose thread cannot be created are drawn here afterwards.
        let mut unstarted = Vec::new();
        std::thread::scope(|scope| {
            for (band, rows) in panic_free::chunks_mut(out, band_bytes).enumerate() {
                let spawned =
                    std::thread::Builder::new().spawn_scoped(scope, move || draw(rows, band));
                if let Err(_) = spawned {
                    unstarted.push(band);
                }
            }
        });
        for (band, rows) in panic_free::chunks_mut(out, band_bytes).enumerate() {
            if unstarted.contains(&band) {
                draw(rows, band);
            }
        }
    }
}

/// One item prepared for compositing.
struct Layer {
    item: Item,
    effects: Vec<Effect>,
    pixels: Option<Arc<RgbaImage>>,
    exact: bool,
    /// Resampled pixels per item pixel.
    sx: f64,
    sy: f64,
    /// Output pixels the item covers.
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
}

impl Layer {
    /// Composites the layer over `rows`, output rows starting at `first_row`.
    fn draw(&self, rows: &mut [u8], first_row: u32, width: u32, region: Rect, scale: f64) {
        let item = &self.item;
        let row_bytes = (width as usize).saturating_mul(4);
        let row_count = rows.len().checked_div(row_bytes).unwrap_or(0) as u32;
        let y0 = self.y0.max(first_row);
        let y1 = self.y1.min(first_row.saturating_add(row_count));
        // Canvas → item-local mapping, computed once per item (see Item::to_local).
        let frame = item.frame();
        let center = frame.center();
        let (sin, cos) = (-item.rotation).to_radians().sin_cos();
        let shift = item.shift();
        let to_local = |p: Point| {
            let (dx, dy) = (p.x - shift.x - center.x, p.y - shift.y - center.y);
            Point::new(
                center.x + dx * cos - dy * sin - frame.x,
                center.y + dx * sin + dy * cos - frame.y,
            )
        };
        for py in y0..y1 {
            for px in self.x0..self.x1 {
                let p = Point::new(
                    region.x + (f64::from(px) + 0.5) / scale,
                    region.y + (f64::from(py) + 0.5) / scale,
                );
                let local = to_local(p);
                let (lx, ly) = (local.x * self.sx, local.y * self.sy);
                let color = match &self.pixels {
                    Some(pixels) => {
                        if self.exact {
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
                color[3] *= opacity(item, &self.effects, p) as f32;
                if color[3] <= 0.0 {
                    continue;
                }
                let at = (py.saturating_sub(first_row) as usize)
                    .saturating_mul(row_bytes)
                    .saturating_add((px as usize).saturating_mul(4));
                let Some(dst) = rows.get_mut(at..at.saturating_add(4)) else {
                    continue;
                };
                let mut below = [0f32; 4];
                for (b, d) in below.iter_mut().zip(dst.iter()) {
                    *b = f32::from(*d) / 255.0;
                }
                let mixed = over(color, below);
                for (d, v) in dst.iter_mut().zip(mixed) {
                    *d = to_u8(v);
                }
            }
        }
    }
}

/// Crops `source`, resamples it to the size in `bits` (see [`ItemKey::bits`]) and applies
/// the light settings at that size (LIGHT-005).
fn resample(source: &RgbaImage, bits: ItemKeyBits, light: Option<Light>) -> RgbaImage {
    let (left, top, crop_w, crop_h, width, height, filter, ..) = bits;
    let cropped = image::imageops::crop_imm(
        source,
        left,
        top,
        crop_w.min(source.width().saturating_sub(left)),
        crop_h.min(source.height().saturating_sub(top)),
    )
    .to_image();
    let mut pixels = if cropped.dimensions() == (width, height) {
        cropped
    } else {
        image::imageops::resize(&cropped, width, height, filter_type(filter))
    };
    if let Some(light) = light {
        let scale = (f64::from(width) / f64::from(crop_w.max(1))
            + f64::from(height) / f64::from(crop_h.max(1)))
            / 2.0;
        light::apply(&mut pixels, &light, scale);
    }
    pixels
}

/// The source with brush strokes applied at full resolution (SMEAR-005, DODGE-004).
fn smear(source: &RgbaImage, strokes: &[Stroke]) -> RgbaImage {
    let mut pixels = source.clone();
    for stroke in strokes {
        retouch::apply(&mut pixels, stroke, |p| p, 1.0);
    }
    pixels
}

fn to_u8(v: f32) -> u8 {
    (v.clamped(0.0, 1.0) * 255.0).round() as u8
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
    let p = image.get_pixel_checked(x, y).map_or([0; 4], |p| p.0);
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
    let Some(item) = state.items.get(index) else {
        return Vec::new();
    };
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
