//! Project state and the steps that change it. A project's state is the result of applying
//! its steps, in order, to an initial canvas.

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::geometry::{PixelRect, Point, Rect, rotated_bounds};

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
        let t = t.clamp(0.0, 1.0);
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

    /// Axis-aligned bounds of the item as drawn (after rotation).
    pub fn bounds(&self) -> Rect {
        let frame = self.frame();
        rotated_bounds(frame.center(), frame.width, frame.height, self.rotation)
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
        let q = p.rotated_about(frame.center(), -self.rotation);
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
}

impl ProjectState {
    pub fn new(canvas: Canvas, sources: BTreeMap<SourceId, PathBuf>) -> Self {
        Self {
            canvas,
            sources,
            items: Vec::new(),
            transitions: Vec::new(),
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
        ItemId(self.items.iter().map(|i| i.id.0).max().unwrap_or(0) + 1)
    }

    pub fn next_transition_id(&self) -> TransitionId {
        TransitionId(self.transitions.iter().map(|t| t.id.0).max().unwrap_or(0) + 1)
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
                let moved = self.items.remove(from);
                let index = index.min(self.items.len());
                self.items.insert(index, moved);
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
                    item.x += offset_x;
                    item.y += offset_y;
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
                self.transitions.remove(index);
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
        }
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
