//! Project state and the steps that change it. A project's state is the result of applying
//! its steps, in order, to an initial canvas.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::geometry::{PixelRect, Point, Rect, rotated_bounds};
use crate::panic_free;
use crate::panic_free::Clamped;

/// One more than the largest id in `used`, or the lowest free id once that would
/// overflow.
pub(crate) fn next_id(used: impl Iterator<Item = u32> + Clone) -> u32 {
    match used.clone().max().unwrap_or(0).checked_add(1) {
        Some(id) => id,
        None => {
            let used: BTreeSet<u32> = used.collect();
            (1..=u32::MAX)
                .find(|id| !used.contains(id))
                .unwrap_or(u32::MAX)
        }
    }
}

macro_rules! id_type {
    ($name:ident, $prefix:literal) => {
        #[derive(
            Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub u32);

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!($prefix, "{}"), self.0)
            }
        }
    };
}

id_type!(SourceId, "source ");
id_type!(ItemId, "item ");
id_type!(TransitionId, "transition ");

/// Resampling filter (RES-002).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Filter {
    Nearest,
    Bilinear,
    #[default]
    Lanczos3,
}

impl Filter {
    pub const ALL: [Filter; 3] = [Filter::Nearest, Filter::Bilinear, Filter::Lanczos3];

    pub fn label(self) -> &'static str {
        match self {
            Filter::Nearest => "Nearest",
            Filter::Bilinear => "Bilinear",
            Filter::Lanczos3 => "Lanczos3",
        }
    }
}

/// Shape of a transparency ramp (BLEND-003, BLEND-007).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Curve {
    #[default]
    Linear,
    Smooth,
}

impl Curve {
    pub const ALL: [Curve; 2] = [Curve::Linear, Curve::Smooth];

    /// Maps `t` in `0..=1` (clamped) to an opacity in `0..=1`.
    pub fn apply(self, t: f64) -> f64 {
        let t = t.clamped(0.0, 1.0);
        match self {
            Curve::Linear => t,
            Curve::Smooth => t * t * (3.0 - 2.0 * t),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Curve::Linear => "Linear",
            Curve::Smooth => "Smooth",
        }
    }
}

/// Direction of a gradient seam (see glossary: horizontal and vertical transitions).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Orientation {
    /// Items side by side; vertical seam line; blend runs left to right.
    Horizontal,
    /// Items above and below; horizontal seam line; blend runs top to bottom.
    Vertical,
}

impl Orientation {
    pub fn label(self) -> &'static str {
        match self {
            Orientation::Horizontal => "Horizontal",
            Orientation::Vertical => "Vertical",
        }
    }
}

/// Feathered edges of one item (BLEND-007).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Feather {
    pub left: bool,
    pub right: bool,
    pub top: bool,
    pub bottom: bool,
    /// Band width in canvas pixels.
    pub width: f64,
    pub curve: Curve,
}

impl Feather {
    pub fn any_edge(&self) -> bool {
        self.left || self.right || self.top || self.bottom
    }
}

/// What a brush stroke does to the pixels under the brush: the smear kinds
/// (SMEAR-002) and dodge and burn (DODGE-002).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StrokeKind {
    #[default]
    Smudge,
    Blur,
    Push,
    Dodge,
    Burn,
}

impl StrokeKind {
    /// The smear tool's kinds (SMEAR-002).
    pub const SMEAR: [StrokeKind; 3] = [StrokeKind::Smudge, StrokeKind::Blur, StrokeKind::Push];
    /// The dodge and burn tool's kinds (DODGE-002).
    pub const DODGE_BURN: [StrokeKind; 2] = [StrokeKind::Dodge, StrokeKind::Burn];

    pub fn label(self) -> &'static str {
        match self {
            StrokeKind::Smudge => "Smudge",
            StrokeKind::Blur => "Blur",
            StrokeKind::Push => "Push",
            StrokeKind::Dodge => "Dodge",
            StrokeKind::Burn => "Burn",
        }
    }

    pub fn is_smear(self) -> bool {
        StrokeKind::SMEAR.contains(&self)
    }
}

/// The pixels dodge and burn changes (DODGE-002).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToneRange {
    Shadows,
    #[default]
    Midtones,
    Highlights,
}

impl ToneRange {
    pub const ALL: [ToneRange; 3] = [
        ToneRange::Shadows,
        ToneRange::Midtones,
        ToneRange::Highlights,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ToneRange::Shadows => "Shadows",
            ToneRange::Midtones => "Midtones",
            ToneRange::Highlights => "Highlights",
        }
    }
}

/// One brush stroke on an item (SMEAR-004, DODGE-004). Coordinates are pixels of the
/// upright source image, so the stroke stays with the image (SMEAR-005).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stroke {
    pub kind: StrokeKind,
    /// Brush diameter in source pixels.
    pub size: f64,
    /// `0..=1`.
    pub strength: f64,
    /// The brush path, as `[x, y]` source pixels.
    pub points: Vec<[f64; 2]>,
    /// Dodge and burn only: the pixels changed (DODGE-002).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<ToneRange>,
}

/// Light settings of one item (LIGHT-002 to LIGHT-004).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Light {
    /// Stops, `-5..=5` (LIGHT-002).
    pub exposure: f64,
    /// `0..=1` (LIGHT-003).
    pub shadows: f64,
    /// `0..=1` (LIGHT-003).
    pub highlights: f64,
    /// Size of the area each pixel is judged by, in source pixels (LIGHT-003).
    pub radius: f64,
    /// Levels black point, white point and midtones (LIGHT-004).
    pub black: f64,
    pub white: f64,
    pub midtones: f64,
}

impl Light {
    /// Settings that change nothing, with a radius for shadows and highlights.
    pub fn neutral(radius: f64) -> Self {
        Self {
            exposure: 0.0,
            shadows: 0.0,
            highlights: 0.0,
            radius,
            black: 0.0,
            white: 255.0,
            midtones: 1.0,
        }
    }

    /// Default radius: 5% of the longer side of a `width` × `height` image (LIGHT-003).
    pub fn default_radius(width: u32, height: u32) -> f64 {
        (f64::from(width.max(height)) * 0.05).round().max(1.0)
    }

    /// Whether the settings change no pixel; the radius alone changes nothing.
    pub fn is_neutral(&self) -> bool {
        self.exposure == 0.0
            && self.shadows == 0.0
            && self.highlights == 0.0
            && self.black == 0.0
            && self.white == 255.0
            && self.midtones == 1.0
    }

    /// Why the settings are out of range, if they are.
    pub fn check(&self) -> Result<(), StepError> {
        let within = |v: f64, lo: f64, hi: f64| v.is_finite() && v >= lo && v <= hi;
        if !within(self.exposure, -5.0, 5.0) {
            return Err(invalid("exposure must be from -5 to +5 stops"));
        }
        if !(within(self.shadows, 0.0, 1.0) && within(self.highlights, 0.0, 1.0)) {
            return Err(invalid("shadows and highlights must be from 0% to 100%"));
        }
        if !(self.radius.is_finite() && self.radius >= 1.0) {
            return Err(invalid("radius must be at least 1 pixel"));
        }
        if !(within(self.black, 0.0, 254.0)
            && within(self.white, 1.0, 255.0)
            && self.white > self.black)
        {
            return Err(invalid(
                "levels need a black point below the white point, from 0 to 255",
            ));
        }
        if !within(self.midtones, 0.1, 9.99) {
            return Err(invalid("midtones must be from 0.10 to 9.99"));
        }
        Ok(())
    }
}

/// A transition between two items (BLEND-006).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum TransitionKind {
    GradientSeam {
        orientation: Orientation,
        /// Seam position relative to the center of the overlap, along the blend direction.
        offset: f64,
        /// Band width in canvas pixels.
        width: f64,
        curve: Curve,
    },
    Crossfade {
        /// Opacity of the upper item within the overlap, `0..=1`.
        ratio: f64,
    },
}

impl TransitionKind {
    pub fn label(&self) -> &'static str {
        match self {
            TransitionKind::GradientSeam { .. } => "Gradient seam",
            TransitionKind::Crossfade { .. } => "Crossfade",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transition {
    pub id: TransitionId,
    pub items: [ItemId; 2],
    pub kind: TransitionKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum CanvasSize {
    /// The canvas is the bounding box of the items (a single image's canvas tracks it).
    Fit,
    Fixed {
        width: u32,
        height: u32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Canvas {
    pub size: CanvasSize,
    #[serde(default)]
    pub dpi: Option<f64>,
}

impl Default for Canvas {
    fn default() -> Self {
        Self {
            size: CanvasSize::Fit,
            dpi: None,
        }
    }
}

/// One placement of a source on the canvas.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Item {
    pub id: ItemId,
    pub source: SourceId,
    /// Top-left of the unrotated box, in canvas pixels.
    pub x: i32,
    pub y: i32,
    /// Size of the unrotated box, in canvas pixels.
    pub width: u32,
    pub height: u32,
    /// Size of the (upright) source image.
    pub source_width: u32,
    pub source_height: u32,
    /// Region of the source shown, in source pixels.
    pub crop: PixelRect,
    /// Clockwise rotation about the box center, in degrees.
    pub rotation: f64,
    pub filter: Filter,
    pub feather: Option<Feather>,
    pub metadata_stripped: bool,
    /// `None` until the light is adjusted (LIGHT-006).
    pub light: Option<Light>,
}

impl Item {
    /// The unrotated box.
    pub fn frame(&self) -> Rect {
        Rect::new(
            f64::from(self.x),
            f64::from(self.y),
            f64::from(self.width),
            f64::from(self.height),
        )
    }

    /// Half-pixel shift applied after rotating, so odd quarter turns of a box whose sides
    /// differ by an odd number of pixels stay pixel-aligned (PLACE-007).
    pub fn shift(&self) -> Point {
        match self.quarter_turns() {
            Some(1 | 3) if (self.width ^ self.height) & 1 == 1 => Point::new(0.5, 0.5),
            _ => Point::default(),
        }
    }

    /// Canvas point of a point within the unrotated box.
    pub fn to_canvas(&self, local: Point) -> Point {
        let frame = self.frame();
        let shift = self.shift();
        let p = Point::new(frame.x + local.x, frame.y + local.y)
            .rotated_about(frame.center(), self.rotation);
        Point::new(p.x + shift.x, p.y + shift.y)
    }

    /// Axis-aligned bounds of the item as drawn (after rotation).
    pub fn bounds(&self) -> Rect {
        let frame = self.frame();
        let c = frame.center();
        let shift = self.shift();
        rotated_bounds(
            Point::new(c.x + shift.x, c.y + shift.y),
            frame.width,
            frame.height,
            self.rotation,
        )
    }

    /// Rotation normalized to `0..360`.
    pub fn normalized_rotation(&self) -> f64 {
        self.rotation.rem_euclid(360.0)
    }

    /// Whether the rotation is a whole number of quarter turns (PLACE-007).
    pub fn quarter_turns(&self) -> Option<u32> {
        let r = self.normalized_rotation();
        let turns = (r / 90.0).round();
        ((r - turns * 90.0).abs() < 1e-9).then_some(turns as u32 % 4)
    }

    /// Maps a canvas point to coordinates within the unrotated box.
    pub fn to_local(&self, p: Point) -> Point {
        let frame = self.frame();
        let shift = self.shift();
        let q =
            Point::new(p.x - shift.x, p.y - shift.y).rotated_about(frame.center(), -self.rotation);
        Point::new(q.x - frame.x, q.y - frame.y)
    }

    /// Whether the item covers a canvas point.
    pub fn contains(&self, p: Point) -> bool {
        let local = self.to_local(p);
        local.x >= 0.0
            && local.y >= 0.0
            && local.x < f64::from(self.width)
            && local.y < f64::from(self.height)
    }
}

/// Changes to a project. Each completed user action records one step (UNDO-004).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Step {
    AddItem {
        id: ItemId,
        source: SourceId,
        x: i32,
        y: i32,
        source_width: u32,
        source_height: u32,
    },
    MoveItem {
        item: ItemId,
        x: i32,
        y: i32,
    },
    ResizeItem {
        item: ItemId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        filter: Filter,
    },
    CropItem {
        item: ItemId,
        crop: PixelRect,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    },
    SetZOrder {
        item: ItemId,
        index: usize,
    },
    RotateItem {
        item: ItemId,
        degrees: f64,
    },
    ResizeCanvas {
        width: u32,
        height: u32,
        /// Items move by this much, so space can be added on the left or top.
        offset_x: i32,
        offset_y: i32,
        dpi: Option<f64>,
    },
    SetDpi {
        dpi: Option<f64>,
    },
    SetPrintSize {
        item: ItemId,
        width: u32,
        height: u32,
        filter: Filter,
        dpi: f64,
    },
    AddTransition {
        id: TransitionId,
        items: [ItemId; 2],
        kind: TransitionKind,
    },
    EditTransition {
        id: TransitionId,
        kind: TransitionKind,
    },
    RemoveTransition {
        id: TransitionId,
    },
    SetFeather {
        item: ItemId,
        feather: Option<Feather>,
    },
    StripMetadata {
        item: ItemId,
    },
    RelinkSource {
        source: SourceId,
        path: PathBuf,
    },
    RemoveItem {
        item: ItemId,
    },
    Smear {
        item: ItemId,
        stroke: Stroke,
    },
    /// Replaces the item's light settings (LIGHT-006).
    SetLight {
        item: ItemId,
        light: Light,
    },
    /// A dodge or burn stroke (DODGE-004).
    DodgeBurn {
        item: ItemId,
        stroke: Stroke,
    },
}

impl Step {
    /// Name shown in undo and redo labels (UNDO-003).
    pub fn label(&self) -> &'static str {
        match self {
            Step::AddItem { .. } => "Add image",
            Step::MoveItem { .. } => "Move image",
            Step::ResizeItem { .. } => "Resize image",
            Step::CropItem { .. } => "Crop",
            Step::SetZOrder { .. } => "Change stacking order",
            Step::RotateItem { .. } => "Rotate image",
            Step::ResizeCanvas { .. } => "Resize canvas",
            Step::SetDpi { .. } => "Set DPI",
            Step::SetPrintSize { .. } => "Set print size",
            Step::AddTransition { .. } => "Add transition",
            Step::EditTransition { .. } => "Edit transition",
            Step::RemoveTransition { .. } => "Remove transition",
            Step::SetFeather { .. } => "Feather edges",
            Step::StripMetadata { .. } => "Strip metadata",
            Step::RelinkSource { .. } => "Locate source",
            Step::RemoveItem { .. } => "Remove image",
            Step::Smear { .. } => "Smear",
            Step::SetLight { .. } => "Adjust light",
            Step::DodgeBurn { stroke, .. } => match stroke.kind {
                StrokeKind::Burn => "Burn",
                _ => "Dodge",
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum StepError {
    UnknownSource(SourceId),
    UnknownItem(ItemId),
    UnknownTransition(TransitionId),
    DuplicateItem(ItemId),
    DuplicateTransition(TransitionId),
    Invalid(String),
}

impl fmt::Display for StepError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StepError::UnknownSource(id) => write!(f, "unknown {id}"),
            StepError::UnknownItem(id) => write!(f, "unknown {id}"),
            StepError::UnknownTransition(id) => write!(f, "unknown {id}"),
            StepError::DuplicateItem(id) => write!(f, "{id} already exists"),
            StepError::DuplicateTransition(id) => write!(f, "{id} already exists"),
            StepError::Invalid(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for StepError {}

fn invalid(message: impl Into<String>) -> StepError {
    StepError::Invalid(message.into())
}

/// Geometry of a transition, derived from its items' current positions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransitionGeometry {
    pub lower: ItemId,
    pub upper: ItemId,
    pub overlap: Rect,
    /// For a gradient seam: the seam position on the canvas along the blend direction,
    /// and whether the upper item is opaque on the low-coordinate side of it.
    pub seam: Option<SeamGeometry>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeamGeometry {
    pub orientation: Orientation,
    pub position: f64,
    pub width: f64,
    pub curve: Curve,
    pub upper_before: bool,
}

impl SeamGeometry {
    pub fn band_start(&self) -> f64 {
        self.position - self.width / 2.0
    }

    pub fn band_end(&self) -> f64 {
        self.position + self.width / 2.0
    }

    /// Opacity multiplier for the upper item at a canvas point.
    pub fn opacity(&self, p: Point) -> f64 {
        let along = match self.orientation {
            Orientation::Horizontal => p.x,
            Orientation::Vertical => p.y,
        };
        let t = if self.width <= 0.0 {
            if along < self.position { 0.0 } else { 1.0 }
        } else {
            (along - self.band_start()) / self.width
        };
        let t = if self.upper_before { 1.0 - t } else { t };
        self.curve.apply(t)
    }
}

/// The result of applying a project's steps.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProjectState {
    pub canvas: Canvas,
    pub sources: BTreeMap<SourceId, PathBuf>,
    /// Bottom to top (z-order).
    pub items: Vec<Item>,
    pub transitions: Vec<Transition>,
    /// Smear strokes of each item, in the order they were made (SMEAR-004).
    pub strokes: BTreeMap<ItemId, Vec<Stroke>>,
}

impl ProjectState {
    pub fn new(canvas: Canvas, sources: BTreeMap<SourceId, PathBuf>) -> Self {
        Self {
            canvas,
            sources,
            items: Vec::new(),
            transitions: Vec::new(),
            strokes: BTreeMap::new(),
        }
    }

    pub fn item(&self, id: ItemId) -> Option<&Item> {
        self.items.iter().find(|item| item.id == id)
    }

    pub fn item_index(&self, id: ItemId) -> Option<usize> {
        self.items.iter().position(|item| item.id == id)
    }

    pub fn transition(&self, id: TransitionId) -> Option<&Transition> {
        self.transitions.iter().find(|t| t.id == id)
    }

    pub fn next_item_id(&self) -> ItemId {
        ItemId(next_id(self.items.iter().map(|i| i.id.0)))
    }

    pub fn next_transition_id(&self) -> TransitionId {
        TransitionId(next_id(self.transitions.iter().map(|t| t.id.0)))
    }

    /// The canvas area, in canvas coordinates.
    pub fn canvas_rect(&self) -> Rect {
        match self.canvas.size {
            CanvasSize::Fixed { width, height } => {
                Rect::new(0.0, 0.0, f64::from(width), f64::from(height))
            }
            CanvasSize::Fit => self
                .items
                .iter()
                .map(Item::bounds)
                .reduce(|a, b| a.union(&b))
                .map(|r| r.round_out())
                .unwrap_or_default(),
        }
    }

    /// Pixel size of the canvas.
    pub fn canvas_size(&self) -> (u32, u32) {
        let r = self.canvas_rect();
        (r.width as u32, r.height as u32)
    }

    /// The item's smear strokes, oldest first.
    pub fn strokes_of(&self, item: ItemId) -> &[Stroke] {
        self.strokes.get(&item).map_or(&[], Vec::as_slice)
    }

    pub fn transitions_of(&self, item: ItemId) -> impl Iterator<Item = &Transition> {
        self.transitions
            .iter()
            .filter(move |t| t.items.contains(&item))
    }

    pub fn transition_between(&self, a: ItemId, b: ItemId) -> Option<&Transition> {
        self.transitions
            .iter()
            .find(|t| t.items.contains(&a) && t.items.contains(&b))
    }

    /// Overlap of two items' bounds, if any.
    pub fn overlap(&self, a: ItemId, b: ItemId) -> Option<Rect> {
        let a = self.item(a)?;
        let b = self.item(b)?;
        a.bounds().intersection(&b.bounds())
    }

    /// Current geometry of a transition, or `None` if its items do not overlap
    /// (BLEND-005: the transition is kept but has no effect).
    pub fn transition_geometry(&self, transition: &Transition) -> Option<TransitionGeometry> {
        let [a, b] = transition.items;
        let overlap = self.overlap(a, b)?;
        let (ia, ib) = (self.item_index(a)?, self.item_index(b)?);
        let (lower, upper) = if ia < ib { (a, b) } else { (b, a) };
        let seam = match transition.kind {
            TransitionKind::GradientSeam {
                orientation,
                offset,
                width,
                curve,
            } => {
                let upper_center = self.item(upper)?.bounds().center();
                let lower_center = self.item(lower)?.bounds().center();
                let (center, upper_before) = match orientation {
                    Orientation::Horizontal => {
                        (overlap.center().x, upper_center.x <= lower_center.x)
                    }
                    Orientation::Vertical => (overlap.center().y, upper_center.y <= lower_center.y),
                };
                Some(SeamGeometry {
                    orientation,
                    position: center + offset,
                    width,
                    curve,
                    upper_before,
                })
            }
            TransitionKind::Crossfade { .. } => None,
        };
        Some(TransitionGeometry {
            lower,
            upper,
            overlap,
            seam,
        })
    }

    fn item_mut(&mut self, id: ItemId) -> Result<&mut Item, StepError> {
        self.items
            .iter_mut()
            .find(|item| item.id == id)
            .ok_or(StepError::UnknownItem(id))
    }

    /// Absolute seam positions of the gradient seams attached to `item`.
    fn seam_positions(&self, item: ItemId) -> Vec<(TransitionId, f64)> {
        self.transitions_of(item)
            .filter_map(|t| Some((t.id, self.transition_geometry(t)?.seam?.position)))
            .collect()
    }

    /// Keeps seams at fixed canvas positions after their items' bounds changed.
    fn pin_seams(&mut self, positions: Vec<(TransitionId, f64)>) {
        for (id, position) in positions {
            let Some(transition) = self.transition(id).copied() else {
                continue;
            };
            let Some(geometry) = self.transition_geometry(&transition) else {
                continue;
            };
            let Some(seam) = geometry.seam else { continue };
            let center = seam.position
                - match transition.kind {
                    TransitionKind::GradientSeam { offset, .. } => offset,
                    TransitionKind::Crossfade { .. } => 0.0,
                };
            if let Some(t) = self.transitions.iter_mut().find(|t| t.id == id)
                && let TransitionKind::GradientSeam { offset, .. } = &mut t.kind
            {
                *offset = position - center;
            }
        }
    }

    /// Applies one step, or leaves the state unchanged and returns why it is invalid.
    pub fn apply(&mut self, step: &Step) -> Result<(), StepError> {
        let mut next = self.clone();
        next.apply_in_place(step)?;
        *self = next;
        Ok(())
    }

    fn apply_in_place(&mut self, step: &Step) -> Result<(), StepError> {
        match *step {
            Step::AddItem {
                id,
                source,
                x,
                y,
                source_width,
                source_height,
            } => {
                if !self.sources.contains_key(&source) {
                    return Err(StepError::UnknownSource(source));
                }
                if self.item(id).is_some() {
                    return Err(StepError::DuplicateItem(id));
                }
                check_size(source_width, source_height)?;
                self.items.push(Item {
                    id,
                    source,
                    x,
                    y,
                    width: source_width,
                    height: source_height,
                    source_width,
                    source_height,
                    crop: PixelRect::new(0, 0, source_width, source_height),
                    rotation: 0.0,
                    filter: Filter::default(),
                    feather: None,
                    metadata_stripped: false,
                    light: None,
                });
            }
            Step::MoveItem { item, x, y } => {
                let item = self.item_mut(item)?;
                item.x = x;
                item.y = y;
            }
            Step::ResizeItem {
                item,
                x,
                y,
                width,
                height,
                filter,
            } => {
                check_size(width, height)?;
                let item = self.item_mut(item)?;
                scale_feather(item, width, height);
                item.x = x;
                item.y = y;
                item.width = width;
                item.height = height;
                item.filter = filter;
            }
            Step::CropItem {
                item,
                crop,
                x,
                y,
                width,
                height,
            } => {
                check_size(width, height)?;
                check_size(crop.width, crop.height)?;
                let item = self.item_mut(item)?;
                if crop.right() > item.source_width || crop.bottom() > item.source_height {
                    return Err(invalid("crop extends outside the image"));
                }
                item.crop = crop;
                item.x = x;
                item.y = y;
                item.width = width;
                item.height = height;
            }
            Step::SetZOrder { item, index } => {
                let from = self.item_index(item).ok_or(StepError::UnknownItem(item))?;
                let moved = panic_free::remove_at(&mut self.items, from)
                    .ok_or(StepError::UnknownItem(item))?;
                let index = index.min(self.items.len());
                if let Err(moved) = panic_free::insert_at(&mut self.items, index, moved) {
                    self.items.push(moved);
                }
            }
            Step::RotateItem { item, degrees } => {
                if !degrees.is_finite() {
                    return Err(invalid("rotation must be a number"));
                }
                let pinned = self.seam_positions(item);
                self.item_mut(item)?.rotation = degrees;
                self.pin_seams(pinned);
            }
            Step::ResizeCanvas {
                width,
                height,
                offset_x,
                offset_y,
                dpi,
            } => {
                check_size(width, height)?;
                if let Some(dpi) = dpi {
                    check_dpi(dpi)?;
                }
                self.canvas.dpi = dpi;
                self.canvas.size = CanvasSize::Fixed { width, height };
                for item in &mut self.items {
                    item.x = item.x.saturating_add(offset_x);
                    item.y = item.y.saturating_add(offset_y);
                }
            }
            Step::SetDpi { dpi } => {
                if let Some(dpi) = dpi {
                    check_dpi(dpi)?;
                }
                self.canvas.dpi = dpi;
            }
            Step::SetPrintSize {
                item,
                width,
                height,
                filter,
                dpi,
            } => {
                check_size(width, height)?;
                check_dpi(dpi)?;
                let item = self.item_mut(item)?;
                scale_feather(item, width, height);
                item.width = width;
                item.height = height;
                item.filter = filter;
                self.canvas.dpi = Some(dpi);
            }
            Step::AddTransition { id, items, kind } => {
                if self.transition(id).is_some() {
                    return Err(StepError::DuplicateTransition(id));
                }
                for item in items {
                    self.item(item).ok_or(StepError::UnknownItem(item))?;
                }
                if items[0] == items[1] {
                    return Err(invalid("a transition needs two different images"));
                }
                if self.transition_between(items[0], items[1]).is_some() {
                    return Err(invalid("these images already have a transition"));
                }
                check_kind(&kind)?;
                self.transitions.push(Transition { id, items, kind });
            }
            Step::EditTransition { id, kind } => {
                check_kind(&kind)?;
                self.transitions
                    .iter_mut()
                    .find(|t| t.id == id)
                    .ok_or(StepError::UnknownTransition(id))?
                    .kind = kind;
            }
            Step::RemoveTransition { id } => {
                let index = self
                    .transitions
                    .iter()
                    .position(|t| t.id == id)
                    .ok_or(StepError::UnknownTransition(id))?;
                panic_free::remove_at(&mut self.transitions, index)
                    .ok_or(StepError::UnknownTransition(id))?;
            }
            Step::SetFeather { item, feather } => {
                if let Some(f) = feather
                    && !(f.width.is_finite() && f.width > 0.0)
                {
                    return Err(invalid("feather width must be positive"));
                }
                self.item_mut(item)?.feather = feather;
            }
            Step::StripMetadata { item } => {
                self.item_mut(item)?.metadata_stripped = true;
            }
            Step::RelinkSource { source, ref path } => {
                let entry = self
                    .sources
                    .get_mut(&source)
                    .ok_or(StepError::UnknownSource(source))?;
                *entry = path.clone();
            }
            Step::RemoveItem { item } => {
                let index = self.item_index(item).ok_or(StepError::UnknownItem(item))?;
                panic_free::remove_at(&mut self.items, index)
                    .ok_or(StepError::UnknownItem(item))?;
                self.transitions.retain(|t| !t.items.contains(&item));
                self.strokes.remove(&item);
            }
            Step::Smear { item, ref stroke } => {
                if !stroke.kind.is_smear() || stroke.range.is_some() {
                    return Err(invalid("a smear stroke needs a smear kind and no range"));
                }
                self.add_stroke(item, stroke)?;
            }
            Step::DodgeBurn { item, ref stroke } => {
                if stroke.kind.is_smear() || stroke.range.is_none() {
                    return Err(invalid("a dodge or burn stroke needs its kind and range"));
                }
                self.add_stroke(item, stroke)?;
            }
            Step::SetLight { item, light } => {
                light.check()?;
                self.item_mut(item)?.light = Some(light);
            }
        }
        Ok(())
    }
}

impl ProjectState {
    /// Checks a brush stroke and adds it to the item's strokes (SMEAR-004, DODGE-004).
    fn add_stroke(&mut self, item: ItemId, stroke: &Stroke) -> Result<(), StepError> {
        self.item(item).ok_or(StepError::UnknownItem(item))?;
        let finite = stroke.points.iter().flatten().all(|v| v.is_finite());
        if !(finite && stroke.size.is_finite() && stroke.size > 0.0) {
            return Err(invalid("brush size must be positive"));
        }
        if !(0.0..=1.0).contains(&stroke.strength) || stroke.points.is_empty() {
            return Err(invalid(
                "a stroke needs points and a strength from 0% to 100%",
            ));
        }
        self.strokes.entry(item).or_default().push(stroke.clone());
        Ok(())
    }
}

fn check_size(width: u32, height: u32) -> Result<(), StepError> {
    if width == 0 || height == 0 {
        return Err(invalid("width and height must be at least 1 pixel"));
    }
    Ok(())
}

fn check_dpi(dpi: f64) -> Result<(), StepError> {
    if !(dpi.is_finite() && dpi > 0.0) {
        return Err(invalid("DPI must be positive"));
    }
    Ok(())
}

fn check_kind(kind: &TransitionKind) -> Result<(), StepError> {
    match *kind {
        TransitionKind::GradientSeam { offset, width, .. } => {
            if !(offset.is_finite() && width.is_finite() && width >= 0.0) {
                return Err(invalid("seam width must not be negative"));
            }
        }
        TransitionKind::Crossfade { ratio } => {
            if !(0.0..=1.0).contains(&ratio) {
                return Err(invalid("crossfade ratio must be between 0% and 100%"));
            }
        }
    }
    Ok(())
}

/// Feathered edges resize with their item (BLEND-007).
fn scale_feather(item: &mut Item, width: u32, height: u32) {
    if let Some(feather) = &mut item.feather {
        let sx = f64::from(width) / f64::from(item.width);
        let sy = f64::from(height) / f64::from(item.height);
        feather.width *= (sx + sy) / 2.0;
    }
}
