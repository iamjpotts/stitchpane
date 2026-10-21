//! Pointer input: hit testing, drags (PLACE-002, PLACE-003, PLACE-008, BLEND-002,
//! BLEND-007, CANVAS-003, CROP-001), snapping (PLACE-006) and context menus (UI-004).

use super::crop::ItemMapping;
use super::{Command, Edge, Editor, MenuEntry, Modifiers, Selection, ZOrder, round_tenth};
use crate::geometry::{Point, Rect};
use crate::model::{
    CanvasSize, Item, ItemId, Orientation, SeamGeometry, Step, TransitionId, TransitionKind,
};

/// Half the size of a square handle, in screen pixels.
pub(super) const HANDLE_RADIUS: f64 = 5.0;
/// How close the pointer must be to a line to grab it, in screen pixels.
const LINE_TOLERANCE: f64 = 5.0;
/// Distance of the rotation handle above the item's top edge, in screen pixels.
pub(super) const ROTATE_OFFSET: f64 = 28.0;
/// Snapping distance, in screen pixels (PLACE-006).
pub const SNAP_DISTANCE: f64 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Primary,
    Secondary,
}

/// Pointer shape hint for the UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cursor {
    Default,
    Move,
    Grab,
    ResizeHorizontal,
    ResizeVertical,
    ResizeDiagonal,
    Rotate,
}

/// Corner and edge handles of a box, clockwise from the top-left.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HandlePos {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
}

impl HandlePos {
    pub(super) const ALL: [HandlePos; 8] = [
        HandlePos::TopLeft,
        HandlePos::Top,
        HandlePos::TopRight,
        HandlePos::Right,
        HandlePos::BottomRight,
        HandlePos::Bottom,
        HandlePos::BottomLeft,
        HandlePos::Left,
    ];

    /// Position within a `w` × `h` box.
    pub(super) fn at(self, w: f64, h: f64) -> Point {
        let (fx, fy) = match self {
            HandlePos::TopLeft => (0.0, 0.0),
            HandlePos::Top => (0.5, 0.0),
            HandlePos::TopRight => (1.0, 0.0),
            HandlePos::Right => (1.0, 0.5),
            HandlePos::BottomRight => (1.0, 1.0),
            HandlePos::Bottom => (0.5, 1.0),
            HandlePos::BottomLeft => (0.0, 1.0),
            HandlePos::Left => (0.0, 0.5),
        };
        Point::new(fx * w, fy * h)
    }

    fn left(self) -> bool {
        matches!(
            self,
            HandlePos::TopLeft | HandlePos::Left | HandlePos::BottomLeft
        )
    }

    fn right(self) -> bool {
        matches!(
            self,
            HandlePos::TopRight | HandlePos::Right | HandlePos::BottomRight
        )
    }

    fn top(self) -> bool {
        matches!(
            self,
            HandlePos::TopLeft | HandlePos::Top | HandlePos::TopRight
        )
    }

    fn bottom(self) -> bool {
        matches!(
            self,
            HandlePos::BottomLeft | HandlePos::Bottom | HandlePos::BottomRight
        )
    }

    fn is_corner(self) -> bool {
        (self.left() || self.right()) && (self.top() || self.bottom())
    }

    fn cursor(self) -> Cursor {
        match self {
            HandlePos::Left | HandlePos::Right => Cursor::ResizeHorizontal,
            HandlePos::Top | HandlePos::Bottom => Cursor::ResizeVertical,
            _ => Cursor::ResizeDiagonal,
        }
    }
}

/// Parts of a gradient seam that can be dragged (BLEND-002).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SeamPart {
    Seam,
    BandStart,
    BandEnd,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Hit {
    CropHandle(HandlePos),
    CropInside,
    Rotate(ItemId),
    Resize(ItemId, HandlePos),
    Feather(ItemId, Edge),
    Seam(TransitionId, SeamPart),
    Canvas(HandlePos),
    Item(ItemId),
    Empty,
}

/// A snapping guide line (PLACE-006), in canvas coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Guide {
    /// A vertical line at `x = at`, or a horizontal line at `y = at`.
    pub vertical: bool,
    pub at: f64,
}

#[derive(Clone, Debug)]
pub(super) enum GestureKind {
    Move(Item),
    Resize(Item, HandlePos),
    Rotate {
        item: Item,
        start_angle: f64,
    },
    Seam {
        id: TransitionId,
        part: SeamPart,
        kind: TransitionKind,
        seam: SeamGeometry,
    },
    Feather(Item, Edge),
    Canvas {
        handle: HandlePos,
        original: Rect,
    },
    CropHandle {
        handle: HandlePos,
        original: Rect,
        item: Item,
    },
    CropMove {
        original: Rect,
        item: Item,
    },
}

#[derive(Clone, Debug)]
pub(super) struct Gesture {
    start: Point,
    kind: GestureKind,
    preview: Option<Step>,
    /// View scroll that keeps items still on screen while the canvas grows (CANVAS-003).
    pub(super) pan_shift: Point,
    pub(super) guides: Vec<Guide>,
}

impl Gesture {
    pub(super) fn preview(&self) -> Option<&Step> {
        self.preview.as_ref()
    }
}

/// Canvas point of an item-local point.
pub(super) fn item_point(item: &Item, local: Point) -> Point {
    let frame = item.frame();
    Point::new(frame.x + local.x, frame.y + local.y).rotated_about(frame.center(), item.rotation)
}

/// Screen-space unit vector pointing "up" from an item's top edge.
fn item_up(item: &Item) -> Point {
    let (sin, cos) = item.rotation.to_radians().sin_cos();
    Point::new(sin, -cos)
}

/// Canvas point of the inner edge of a feathered band, at the middle of that edge.
pub(super) fn feather_handle(item: &Item, edge: Edge, width: f64) -> Point {
    let f = item.frame();
    let c = f.center();
    match edge {
        Edge::Left => Point::new(f.x + width, c.y),
        Edge::Right => Point::new(f.right() - width, c.y),
        Edge::Top => Point::new(c.x, f.y + width),
        Edge::Bottom => Point::new(c.x, f.bottom() - width),
    }
}

pub(super) fn feathered_edges(item: &Item) -> Vec<Edge> {
    let Some(f) = item.feather else {
        return Vec::new();
    };
    [
        (f.left, Edge::Left),
        (f.right, Edge::Right),
        (f.top, Edge::Top),
        (f.bottom, Edge::Bottom),
    ]
    .into_iter()
    .filter_map(|(on, edge)| on.then_some(edge))
    .collect()
}

/// Lines of a seam on the canvas: (part, start, end).
pub(super) fn seam_lines(seam: &SeamGeometry, overlap: &Rect) -> [(SeamPart, Point, Point); 3] {
    let line = |at: f64| match seam.orientation {
        Orientation::Horizontal => (Point::new(at, overlap.y), Point::new(at, overlap.bottom())),
        Orientation::Vertical => (Point::new(overlap.x, at), Point::new(overlap.right(), at)),
    };
    let (a, b) = line(seam.position);
    let (c, d) = line(seam.band_start());
    let (e, f) = line(seam.band_end());
    [
        (SeamPart::Seam, a, b),
        (SeamPart::BandStart, c, d),
        (SeamPart::BandEnd, e, f),
    ]
}

fn distance_to_segment(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0)
    };
    p.distance(Point::new(a.x + t * dx, a.y + t * dy))
}

fn angle(center: Point, p: Point) -> f64 {
    (p.y - center.y).atan2(p.x - center.x).to_degrees()
}

/// Finds the target nearest to any candidate, within `tolerance`.
/// Returns the correction to add and the target.
fn snap(candidates: &[f64], targets: &[f64], tolerance: f64) -> Option<(f64, f64)> {
    let mut best: Option<(f64, f64)> = None;
    for &c in candidates {
        for &t in targets {
            let d = t - c;
            if d.abs() <= tolerance && best.is_none_or(|(b, _)| d.abs() < b.abs()) {
                best = Some((d, t));
            }
        }
    }
    best
}

impl Editor {
    fn screen(&self, p: Point) -> Point {
        self.view.to_screen(p)
    }

    /// Transitions whose seams are shown and draggable: those of the selected item, or the
    /// selected transition (BLEND-002).
    pub(super) fn visible_seams(&self) -> Vec<(TransitionId, SeamGeometry, Rect)> {
        let state = self.display_state();
        state
            .transitions
            .iter()
            .filter(|t| match self.selection {
                Selection::Transition(id) => t.id == id,
                selection => selection.item().is_some_and(|i| t.items.contains(&i)),
            })
            .filter_map(|t| {
                let g = state.transition_geometry(t)?;
                Some((t.id, g.seam?, g.overlap))
            })
            .collect()
    }

    fn hit(&self, screen: Point) -> Hit {
        let state = self.display_state();
        let near = |canvas: Point| self.screen(canvas).distance(screen) <= HANDLE_RADIUS + 2.0;

        if let Some(crop) = &self.crop {
            if let Some(item) = state.item(crop.item) {
                let mapping = ItemMapping::new(*item);
                for handle in HandlePos::ALL {
                    let at = handle.at(crop.rect.width, crop.rect.height);
                    let p = Point::new(crop.rect.x + at.x, crop.rect.y + at.y);
                    if near(mapping.source_to_canvas(p)) {
                        return Hit::CropHandle(handle);
                    }
                }
                let source = mapping.canvas_to_source(self.view.to_canvas(screen));
                if crop.rect.contains(source) {
                    return Hit::CropInside;
                }
            }
            return Hit::Empty;
        }

        if let Some(item) = self.selection.item().and_then(|id| state.item(id)) {
            let top = self.screen(item_point(
                item,
                Point::new(f64::from(item.width) / 2.0, 0.0),
            ));
            let up = item_up(item);
            let rotate = Point::new(top.x + up.x * ROTATE_OFFSET, top.y + up.y * ROTATE_OFFSET);
            if rotate.distance(screen) <= HANDLE_RADIUS + 3.0 {
                return Hit::Rotate(item.id);
            }
            for edge in feathered_edges(item) {
                let width = item.feather.map(|f| f.width).unwrap_or(0.0);
                if near(feather_handle(item, edge, width)) {
                    return Hit::Feather(item.id, edge);
                }
            }
            for handle in HandlePos::ALL {
                let local = handle.at(f64::from(item.width), f64::from(item.height));
                if near(item_point(item, local)) {
                    return Hit::Resize(item.id, handle);
                }
            }
        }

        for (id, seam, overlap) in self.visible_seams() {
            for (part, a, b) in seam_lines(&seam, &overlap) {
                if distance_to_segment(screen, self.screen(a), self.screen(b)) <= LINE_TOLERANCE {
                    return Hit::Seam(id, part);
                }
            }
        }

        let canvas = state.canvas_rect();
        if !canvas.is_empty() {
            for handle in HandlePos::ALL {
                let at = handle.at(canvas.width, canvas.height);
                if near(Point::new(canvas.x + at.x, canvas.y + at.y)) {
                    return Hit::Canvas(handle);
                }
            }
        }

        let p = self.view.to_canvas(screen);
        state
            .items
            .iter()
            .rev()
            .find(|item| item.contains(p))
            .map(|item| Hit::Item(item.id))
            .unwrap_or(Hit::Empty)
    }

    /// Pointer shape to show at a screen point.
    pub fn cursor_at(&self, screen: Point) -> Cursor {
        if let Some(g) = &self.gesture {
            return match g.kind {
                GestureKind::Move(_) | GestureKind::CropMove { .. } => Cursor::Move,
                GestureKind::Rotate { .. } => Cursor::Rotate,
                GestureKind::Resize(_, h)
                | GestureKind::Canvas { handle: h, .. }
                | GestureKind::CropHandle { handle: h, .. } => h.cursor(),
                GestureKind::Seam { .. } | GestureKind::Feather(..) => Cursor::Grab,
            };
        }
        match self.hit(screen) {
            Hit::CropHandle(h) | Hit::Resize(_, h) | Hit::Canvas(h) => h.cursor(),
            Hit::CropInside | Hit::Item(_) => Cursor::Move,
            Hit::Rotate(_) => Cursor::Rotate,
            Hit::Feather(..) | Hit::Seam(..) => Cursor::Grab,
            Hit::Empty => Cursor::Default,
        }
    }

    /// Snapping guide lines to draw.
    pub fn guides(&self) -> &[Guide] {
        self.gesture
            .as_ref()
            .map(|g| g.guides.as_slice())
            .unwrap_or(&[])
    }

    /// Press. A secondary press returns the context menu to show (UI-004).
    pub fn pointer_down(
        &mut self,
        screen: Point,
        button: Button,
        _mods: Modifiers,
    ) -> Option<Vec<MenuEntry>> {
        self.commit_pending();
        self.gesture = None;
        let hit = self.hit(screen);
        if button == Button::Secondary {
            return Some(self.context_menu(hit));
        }
        let start = self.view.to_canvas(screen);
        let state = self.project.state();
        let kind = match hit {
            Hit::CropHandle(handle) => {
                let crop = self.crop.as_ref()?;
                GestureKind::CropHandle {
                    handle,
                    original: crop.rect,
                    item: *state.item(crop.item)?,
                }
            }
            Hit::CropInside => {
                let crop = self.crop.as_ref()?;
                GestureKind::CropMove {
                    original: crop.rect,
                    item: *state.item(crop.item)?,
                }
            }
            Hit::Rotate(id) => {
                let item = *state.item(id)?;
                GestureKind::Rotate {
                    item,
                    start_angle: angle(item.frame().center(), start),
                }
            }
            Hit::Resize(id, handle) => GestureKind::Resize(*state.item(id)?, handle),
            Hit::Feather(id, edge) => {
                self.selection = Selection::FeatherEdge(id, edge);
                GestureKind::Feather(*state.item(id)?, edge)
            }
            Hit::Seam(id, part) => {
                let t = *state.transition(id)?;
                let seam = state.transition_geometry(&t)?.seam?;
                self.selection = Selection::Transition(id);
                GestureKind::Seam {
                    id,
                    part,
                    kind: t.kind,
                    seam,
                }
            }
            Hit::Canvas(handle) => GestureKind::Canvas {
                handle,
                original: state.canvas_rect(),
            },
            Hit::Item(id) => {
                let item = *state.item(id)?;
                self.selection = Selection::Item(id);
                GestureKind::Move(item)
            }
            Hit::Empty => {
                if self.crop.is_none() {
                    self.selection = Selection::None;
                }
                return None;
            }
        };
        self.gesture = Some(Gesture {
            start,
            kind,
            preview: None,
            pan_shift: Point::default(),
            guides: Vec::new(),
        });
        None
    }

    /// Snap targets along one axis: canvas edges and center (fixed canvases) and other
    /// items' edges and centers.
    fn snap_targets(&self, vertical_lines: bool, exclude: Option<ItemId>) -> Vec<f64> {
        let state = self.project.state();
        let mut targets = Vec::new();
        let mut push = |r: Rect| {
            if vertical_lines {
                targets.extend([r.x, r.center().x, r.right()]);
            } else {
                targets.extend([r.y, r.center().y, r.bottom()]);
            }
        };
        if matches!(state.canvas.size, CanvasSize::Fixed { .. }) {
            push(state.canvas_rect());
        }
        for item in state.items.iter().filter(|i| Some(i.id) != exclude) {
            push(item.bounds());
        }
        targets
    }

    pub fn pointer_move(&mut self, screen: Point, mods: Modifiers) {
        let Some(gesture) = self.gesture.take() else {
            return;
        };
        let p = self.view.to_canvas(screen);
        let tolerance = SNAP_DISTANCE / self.view.zoom;
        let snapping = self.snapping && !mods.alt;
        let mut guides = Vec::new();
        let (dx, dy) = (p.x - gesture.start.x, p.y - gesture.start.y);
        let mut pan_shift = Point::default();

        let preview = match &gesture.kind {
            GestureKind::Move(item) => {
                let (mut dx, mut dy) = (dx, dy);
                if snapping {
                    let b = item.bounds();
                    let moved = Rect::new(b.x + dx, b.y + dy, b.width, b.height);
                    if let Some((d, at)) = snap(
                        &[moved.x, moved.center().x, moved.right()],
                        &self.snap_targets(true, Some(item.id)),
                        tolerance,
                    ) {
                        dx += d;
                        guides.push(Guide { vertical: true, at });
                    }
                    if let Some((d, at)) = snap(
                        &[moved.y, moved.center().y, moved.bottom()],
                        &self.snap_targets(false, Some(item.id)),
                        tolerance,
                    ) {
                        dy += d;
                        guides.push(Guide {
                            vertical: false,
                            at,
                        });
                    }
                }
                let (x, y) = (item.x + dx.round() as i32, item.y + dy.round() as i32);
                ((x, y) != (item.x, item.y)).then_some(Step::MoveItem {
                    item: item.id,
                    x,
                    y,
                })
            }
            GestureKind::Resize(item, handle) => {
                self.resize_preview(item, *handle, p, gesture.start, mods, snapping, &mut guides)
            }
            GestureKind::Rotate { item, start_angle } => {
                let delta = angle(item.frame().center(), p) - start_angle;
                let mut degrees = item.rotation + delta;
                degrees = if mods.shift {
                    (degrees / 15.0).round() * 15.0
                } else {
                    round_tenth(degrees)
                };
                let degrees = degrees.rem_euclid(360.0);
                (degrees != item.rotation).then_some(Step::RotateItem {
                    item: item.id,
                    degrees,
                })
            }
            GestureKind::Seam {
                id,
                part,
                kind,
                seam,
            } => {
                let TransitionKind::GradientSeam {
                    orientation,
                    offset,
                    width,
                    curve,
                } = *kind
                else {
                    return;
                };
                let horizontal = orientation == Orientation::Horizontal;
                let (along, delta) = if horizontal { (p.x, dx) } else { (p.y, dy) };
                let (offset, width) = match part {
                    SeamPart::Seam => {
                        let mut position = seam.position + delta;
                        if snapping
                            && let Some((d, at)) =
                                snap(&[position], &self.snap_targets(horizontal, None), tolerance)
                        {
                            position += d;
                            guides.push(Guide {
                                vertical: horizontal,
                                at,
                            });
                        }
                        (offset + (position - seam.position).round(), width)
                    }
                    SeamPart::BandStart | SeamPart::BandEnd => {
                        (offset, (2.0 * (along - seam.position).abs()).round())
                    }
                };
                Some(Step::EditTransition {
                    id: *id,
                    kind: TransitionKind::GradientSeam {
                        orientation,
                        offset,
                        width,
                        curve,
                    },
                })
            }
            GestureKind::Feather(item, edge) => {
                let f = item.frame();
                let inward = match edge {
                    Edge::Left => p.x - f.x,
                    Edge::Right => f.right() - p.x,
                    Edge::Top => p.y - f.y,
                    Edge::Bottom => f.bottom() - p.y,
                };
                let max = f.width.min(f.height);
                let mut feather = item
                    .feather
                    .unwrap_or_else(|| self.default_feather(item.id));
                feather.width = inward.round().clamp(1.0, max.max(1.0));
                Some(Step::SetFeather {
                    item: item.id,
                    feather: Some(feather),
                })
            }
            GestureKind::Canvas { handle, original } => {
                let (mut l, mut t, mut r, mut b) =
                    (original.x, original.y, original.right(), original.bottom());
                let snap_edge = |value: f64, vertical: bool, guides: &mut Vec<Guide>| -> f64 {
                    if !snapping {
                        return value;
                    }
                    match snap(&[value], &self.item_edges(vertical), tolerance) {
                        Some((d, at)) => {
                            guides.push(Guide { vertical, at });
                            value + d
                        }
                        None => value,
                    }
                };
                if handle.left() {
                    l = snap_edge(l + dx, true, &mut guides).min(r - 1.0);
                }
                if handle.right() {
                    r = snap_edge(r + dx, true, &mut guides).max(l + 1.0);
                }
                if handle.top() {
                    t = snap_edge(t + dy, false, &mut guides).min(b - 1.0);
                }
                if handle.bottom() {
                    b = snap_edge(b + dy, false, &mut guides).max(t + 1.0);
                }
                let (l, t, r, b) = (l.round(), t.round(), r.round(), b.round());
                let offset_x = -l as i32;
                let offset_y = -t as i32;
                pan_shift = Point::new(f64::from(offset_x), f64::from(offset_y));
                Some(Step::ResizeCanvas {
                    width: (r - l) as u32,
                    height: (b - t) as u32,
                    offset_x,
                    offset_y,
                    dpi: self.project.state().canvas.dpi,
                })
            }
            GestureKind::CropHandle {
                handle,
                original,
                item,
            } => {
                let mapping = ItemMapping::new(*item);
                let mut s = mapping.canvas_to_source(p);
                if snapping && item.rotation == 0.0 {
                    let c = mapping.source_to_canvas(s);
                    let (mut cx, mut cy) = (c.x, c.y);
                    if let Some((d, at)) =
                        snap(&[cx], &self.snap_targets(true, Some(item.id)), tolerance)
                    {
                        cx += d;
                        guides.push(Guide { vertical: true, at });
                    }
                    if let Some((d, at)) =
                        snap(&[cy], &self.snap_targets(false, Some(item.id)), tolerance)
                    {
                        cy += d;
                        guides.push(Guide {
                            vertical: false,
                            at,
                        });
                    }
                    s = mapping.canvas_to_source(Point::new(cx, cy));
                }
                self.drag_crop_handle(*handle, *original, s);
                None
            }
            GestureKind::CropMove { original, item } => {
                let mapping = ItemMapping::new(*item);
                let a = mapping.canvas_to_source(gesture.start);
                let b = mapping.canvas_to_source(p);
                if let Some(crop) = &mut self.crop {
                    crop.rect = *original;
                    crop.translate(b.x - a.x, b.y - a.y);
                }
                None
            }
        };

        self.gesture = Some(Gesture {
            preview,
            pan_shift,
            guides,
            ..gesture
        });
    }

    fn item_edges(&self, vertical: bool) -> Vec<f64> {
        self.project
            .state()
            .items
            .iter()
            .flat_map(|i| {
                let b = i.bounds();
                if vertical {
                    [b.x, b.right()]
                } else {
                    [b.y, b.bottom()]
                }
            })
            .collect()
    }

    fn drag_crop_handle(&mut self, handle: HandlePos, original: Rect, s: Point) {
        let Some(crop) = &mut self.crop else { return };
        let (mut l, mut t, mut r, mut b) =
            (original.x, original.y, original.right(), original.bottom());
        if handle.left() {
            l = s.x;
        }
        if handle.right() {
            r = s.x;
        }
        if handle.top() {
            t = s.y;
        }
        if handle.bottom() {
            b = s.y;
        }
        if let Some(ratio) = crop.ratio() {
            // CROP-003: keep the ratio, anchored on the opposite side.
            let (mut w, mut h) = ((r - l).max(1.0), (b - t).max(1.0));
            if handle.is_corner() {
                if w / h > ratio {
                    h = w / ratio;
                } else {
                    w = h * ratio;
                }
            } else if handle.left() || handle.right() {
                h = w / ratio;
            } else {
                w = h * ratio;
            }
            if handle.left() {
                l = r - w;
            } else if handle.right() {
                r = l + w;
            } else {
                let c = (l + r) / 2.0;
                l = c - w / 2.0;
                r = c + w / 2.0;
            }
            if handle.top() {
                t = b - h;
            } else if handle.bottom() {
                b = t + h;
            } else {
                let c = (t + b) / 2.0;
                t = c - h / 2.0;
                b = c + h / 2.0;
            }
            let lim = crop.limits;
            let fits = l >= f64::from(lim.left)
                && t >= f64::from(lim.top)
                && r <= f64::from(lim.right())
                && b <= f64::from(lim.bottom());
            if !fits {
                return;
            }
        }
        crop.set_edges(
            l.min(r - 1.0),
            t.min(b - 1.0),
            r.max(l + 1.0),
            b.max(t + 1.0),
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn resize_preview(
        &self,
        item: &Item,
        handle: HandlePos,
        p: Point,
        start: Point,
        mods: Modifiers,
        snapping: bool,
        guides: &mut Vec<Guide>,
    ) -> Option<Step> {
        let (w, h) = (f64::from(item.width), f64::from(item.height));
        // Pointer movement in the item's own (unrotated) axes.
        let delta = Point::new(p.x - start.x, p.y - start.y)
            .rotated_about(Point::default(), -item.rotation);
        let (mut l, mut t, mut r, mut b) = (0.0, 0.0, w, h);
        if handle.left() {
            l += delta.x;
        }
        if handle.right() {
            r += delta.x;
        }
        if handle.top() {
            t += delta.y;
        }
        if handle.bottom() {
            b += delta.y;
        }
        if snapping && item.rotation == 0.0 {
            let tolerance = SNAP_DISTANCE / self.view.zoom;
            let (fx, fy) = (f64::from(item.x), f64::from(item.y));
            let xs = self.snap_targets(true, Some(item.id));
            let ys = self.snap_targets(false, Some(item.id));
            for (moves, value) in [(handle.left(), &mut l), (handle.right(), &mut r)] {
                if moves && let Some((d, at)) = snap(&[fx + *value], &xs, tolerance) {
                    *value += d;
                    guides.push(Guide { vertical: true, at });
                }
            }
            for (moves, value) in [(handle.top(), &mut t), (handle.bottom(), &mut b)] {
                if moves && let Some((d, at)) = snap(&[fy + *value], &ys, tolerance) {
                    *value += d;
                    guides.push(Guide {
                        vertical: false,
                        at,
                    });
                }
            }
        }
        let mut nw = (r - l).max(1.0);
        let mut nh = (b - t).max(1.0);
        // PLACE-003: corners keep the aspect ratio unless Shift is held.
        if handle.is_corner() && !mods.shift {
            let (sx, sy) = (nw / w, nh / h);
            let s = if (sx - 1.0).abs() > (sy - 1.0).abs() {
                sx
            } else {
                sy
            };
            nw = (w * s).max(1.0);
            nh = (h * s).max(1.0);
        }
        if handle.left() {
            l = r - nw;
        } else {
            r = l + nw;
        }
        if handle.top() {
            t = b - nh;
        } else {
            b = t + nh;
        }
        let center = item_point(item, Point::new((l + r) / 2.0, (t + b) / 2.0));
        let (width, height) = (nw.round() as u32, nh.round() as u32);
        let x = (center.x - f64::from(width) / 2.0).round() as i32;
        let y = (center.y - f64::from(height) / 2.0).round() as i32;
        ((width, height, x, y) != (item.width, item.height, item.x, item.y)).then_some(
            Step::ResizeItem {
                item: item.id,
                x,
                y,
                width: width.max(1),
                height: height.max(1),
                filter: item.filter,
            },
        )
    }

    /// Release: records the gesture's step, if it changed anything.
    pub fn pointer_up(&mut self, screen: Point, mods: Modifiers) {
        if self.gesture.is_some() {
            self.pointer_move(screen, mods);
        }
        let Some(gesture) = self.gesture.take() else {
            return;
        };
        if let Some(step) = gesture.preview {
            if matches!(step, Step::ResizeCanvas { .. }) {
                self.view.fit = false;
                self.view.pan.x += gesture.pan_shift.x;
                self.view.pan.y += gesture.pan_shift.y;
            }
            self.record(step);
        }
    }

    /// Scroll wheel: Ctrl zooms about the pointer (UI-003); otherwise scrolls.
    pub fn scroll(&mut self, screen: Point, dx: f64, dy: f64, mods: Modifiers) {
        if mods.ctrl {
            let factor = view_zoom_factor(dy);
            self.view.zoom_by(factor, screen);
        } else if mods.shift {
            self.view.scroll(dy * 40.0, dx * 40.0);
        } else {
            self.view.scroll(dx * 40.0, dy * 40.0);
        }
    }

    /// The view as displayed, including scrolling from an in-progress canvas resize.
    pub(super) fn display_view(&self) -> super::view::View {
        let mut view = self.view;
        if let Some(g) = &self.gesture {
            view.pan.x += g.pan_shift.x;
            view.pan.y += g.pan_shift.y;
        }
        view
    }

    fn context_menu(&mut self, hit: Hit) -> Vec<MenuEntry> {
        match hit {
            Hit::Seam(id, _) => {
                self.selection = Selection::Transition(id);
                self.transition_menu(id)
            }
            Hit::Item(id) | Hit::Resize(id, _) | Hit::Rotate(id) | Hit::Feather(id, _) => {
                self.selection = Selection::Item(id);
                self.item_menu(id)
            }
            Hit::CropHandle(_) | Hit::CropInside => vec![
                MenuEntry::new("Apply crop", Command::ApplyCrop),
                MenuEntry::new("Cancel crop", Command::CancelCrop),
            ],
            Hit::Canvas(_) | Hit::Empty => vec![
                MenuEntry::new("Add images…", Command::AddImages),
                MenuEntry::new("Canvas size…", Command::CanvasSize),
                MenuEntry::new("Set DPI…", Command::SetDpi),
            ],
        }
    }

    fn transition_menu(&self, id: TransitionId) -> Vec<MenuEntry> {
        let Some(t) = self.project.state().transition(id) else {
            return Vec::new();
        };
        let switch = match t.kind {
            TransitionKind::GradientSeam { .. } => "Change to crossfade",
            TransitionKind::Crossfade { .. } => "Change to gradient seam",
        };
        vec![
            MenuEntry::new("Properties…", Command::TransitionProperties(id)),
            MenuEntry::new(switch, Command::SwitchTransition(id)),
            MenuEntry::new("Remove", Command::RemoveTransition(id)),
        ]
    }

    /// Item context menu (UI-004).
    pub fn item_menu(&mut self, id: ItemId) -> Vec<MenuEntry> {
        let mut menu = vec![
            MenuEntry::new("Crop", Command::Crop(id)),
            MenuEntry::new("Resize…", Command::Resize(id)),
            MenuEntry::new("Print size…", Command::PrintSize(id)),
            MenuEntry::separator(),
            MenuEntry::new("Rotate 90° clockwise", Command::QuarterTurns(id, 1)),
            MenuEntry::new("Rotate 90° counterclockwise", Command::QuarterTurns(id, -1)),
            MenuEntry::new("Rotate 180°", Command::QuarterTurns(id, 2)),
            MenuEntry::new("Rotate…", Command::RotateBy(id)),
            MenuEntry::separator(),
        ];
        for order in [
            ZOrder::Front,
            ZOrder::Forward,
            ZOrder::Backward,
            ZOrder::Back,
        ] {
            menu.push(MenuEntry::new(order.label(), Command::ZOrder(id, order)));
        }
        menu.push(MenuEntry::separator());
        menu.push(MenuEntry::new("Feather edges…", Command::Feather(id)));
        let state = self.project.state().clone();
        for other in state.items.iter().filter(|o| o.id != id) {
            if state.overlap(id, other.id).is_none() {
                continue;
            }
            let name = self.item_name(other.id);
            match state.transition_between(id, other.id) {
                Some(t) => {
                    let kind = t.kind.label();
                    menu.push(MenuEntry::new(
                        format!("{kind} with {name}…"),
                        Command::TransitionProperties(t.id),
                    ));
                    menu.push(MenuEntry::new(
                        format!("Remove {} with {name}", kind.to_lowercase()),
                        Command::RemoveTransition(t.id),
                    ));
                }
                None => {
                    let pair = [other.id, id];
                    menu.push(MenuEntry::new(
                        format!("Horizontal gradient seam with {name}"),
                        Command::AddSeam(pair, Orientation::Horizontal),
                    ));
                    menu.push(MenuEntry::new(
                        format!("Vertical gradient seam with {name}"),
                        Command::AddSeam(pair, Orientation::Vertical),
                    ));
                    menu.push(MenuEntry::new(
                        format!("Crossfade with {name}"),
                        Command::AddCrossfade(pair),
                    ));
                }
            }
        }
        if !self.exif_entries(id).is_empty() {
            menu.push(MenuEntry::separator());
            menu.push(MenuEntry::new(
                "Show EXIF metadata…",
                Command::ExifViewer(Some(id)),
            ));
        }
        menu
    }
}

/// Zoom factor for a scroll amount (negative scrolls zoom in).
fn view_zoom_factor(dy: f64) -> f64 {
    super::view::ZOOM_STEP.powf(-dy.clamp(-4.0, 4.0))
}
