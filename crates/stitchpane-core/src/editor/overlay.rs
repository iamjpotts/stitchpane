//! The frame shown in the canvas area: the rendered canvas over a checkerboard, plus
//! selection outlines, handles, seams, feathered edges, guides and the crop rectangle.

use image::{Rgba, RgbaImage};

use super::crop::ItemMapping;
use super::pointer::{
    HANDLE_RADIUS, HandlePos, ROTATE_OFFSET, feather_handle, feathered_edges, item_point,
    seam_lines,
};
use super::view::View;
use super::{Edge, Editor, Selection};
use crate::geometry::Point;
use crate::model::{ItemId, ProjectState, Stroke, TransitionKind};
use crate::panic_free::{self, Clamped};

const BACKGROUND: [u8; 4] = [0x5e, 0x5c, 0x64, 0xff];
const CHECK_LIGHT: [u8; 4] = [0xff, 0xff, 0xff, 0xff];
const CHECK_DARK: [u8; 4] = [0xd0, 0xd0, 0xd0, 0xff];
const ACCENT: [u8; 4] = [0x35, 0x84, 0xe4, 0xff];
const WHITE: [u8; 4] = [0xff, 0xff, 0xff, 0xff];
const BORDER: [u8; 4] = [0x24, 0x1f, 0x31, 0xff];
const SEAM: [u8; 4] = [0xff, 0x78, 0x00, 0xff];
const FEATHER: [u8; 4] = [0x2e, 0xc2, 0x7e, 0xff];
const GUIDE: [u8; 4] = [0xc0, 0x61, 0xcb, 0xff];
const CHECK_SIZE: i64 = 8;

/// Viewport size and the canvas rectangle on screen.
pub(super) type BackgroundKey = (u32, u32, [i64; 4]);

pub(super) struct FrameCache {
    state: ProjectState,
    view: View,
    live: Option<(ItemId, Stroke)>,
    image: RgbaImage,
}

impl Editor {
    /// The canvas area as an image the size of the viewport.
    pub fn frame(&mut self) -> RgbaImage {
        let view = self.display_view();
        let state = self.display_state().into_owned();
        let live = self.live_stroke();
        let mut image = match &self.frame_cache {
            Some(cache) if cache.view == view && cache.state == state && cache.live == live => {
                cache.image.clone()
            }
            _ => {
                self.renderer.set_live_stroke(live.clone());
                let base = self.base_frame(&state, &view);
                self.frame_cache = Some(FrameCache {
                    state: state.clone(),
                    view,
                    live,
                    image: base.clone(),
                });
                base
            }
        };
        self.draw_overlays(&mut image, &state, &view);
        image
    }

    fn base_frame(&mut self, state: &ProjectState, view: &View) -> RgbaImage {
        let width = view.viewport.0.round().max(1.0) as u32;
        let height = view.viewport.1.round().max(1.0) as u32;
        let canvas = state.canvas_rect();
        let top_left = view.to_screen(Point::new(canvas.x, canvas.y));
        let bottom_right = view.to_screen(Point::new(canvas.right(), canvas.bottom()));
        let bounds = [
            top_left.x.round() as i64,
            top_left.y.round() as i64,
            bottom_right.x.round() as i64,
            bottom_right.y.round() as i64,
        ];
        let key = (width, height, bounds);
        let mut image = match &self.background {
            Some((cached, image)) if *cached == key => image.clone(),
            _ => {
                let [cx0, cy0, cx1, cy1] = bounds;
                let image = panic_free::image_from_fn(width, height, |x, y| {
                    let (sx, sy) = (i64::from(x), i64::from(y));
                    let inside = sx >= cx0 && sx < cx1 && sy >= cy0 && sy < cy1;
                    Rgba(if !inside {
                        BACKGROUND
                    } else if (sx.saturating_sub(cx0) / CHECK_SIZE)
                        .saturating_add(sy.saturating_sub(cy0) / CHECK_SIZE)
                        % 2
                        == 0
                    {
                        CHECK_LIGHT
                    } else {
                        CHECK_DARK
                    })
                })
                .unwrap_or_default();
                self.background = Some((key, image.clone()));
                image
            }
        };
        self.renderer
            .composite(state, view.visible(), view.zoom, true, &mut image);
        image
    }

    fn draw_overlays(&mut self, image: &mut RgbaImage, state: &ProjectState, view: &View) {
        let screen = |p: Point| view.to_screen(p);
        let canvas = state.canvas_rect();

        // Canvas outline and handles (CANVAS-003).
        if !canvas.is_empty() {
            let corners = [
                Point::new(canvas.x, canvas.y),
                Point::new(canvas.right(), canvas.y),
                Point::new(canvas.right(), canvas.bottom()),
                Point::new(canvas.x, canvas.bottom()),
            ];
            polygon(image, &corners.map(screen), BORDER, None);
            if self.crop.is_none() {
                for handle in HandlePos::ALL {
                    let at = handle.at(canvas.width, canvas.height);
                    let p = screen(Point::new(canvas.x + at.x, canvas.y + at.y));
                    square(image, p, HANDLE_RADIUS - 1.0, CHECK_DARK, BORDER);
                }
            }
        }

        // Seams of the selected item or transition (BLEND-002).
        for (id, seam, overlap) in self.visible_seams() {
            let selected = self.selection == Selection::Transition(id);
            for (part, a, b) in seam_lines(&seam, &overlap) {
                let dash = (part != super::pointer::SeamPart::Seam).then_some(6.0);
                let width = if selected { 2.0 } else { 1.0 };
                line(image, screen(a), screen(b), SEAM, width, dash);
            }
        }
        if let Selection::Transition(id) = self.selection
            && let Some(t) = state.transition(id)
            && let TransitionKind::Crossfade { .. } = t.kind
            && let Some(g) = state.transition_geometry(t)
        {
            let o = g.overlap;
            let corners = [
                Point::new(o.x, o.y),
                Point::new(o.right(), o.y),
                Point::new(o.right(), o.bottom()),
                Point::new(o.x, o.bottom()),
            ];
            polygon(image, &corners.map(screen), SEAM, Some(6.0));
        }

        // Selected item: outline, handles, rotation handle, feathered edges.
        if self.crop.is_none()
            && let Some(item) = self.selection.item().and_then(|id| state.item(id))
        {
            let (w, h) = (f64::from(item.width), f64::from(item.height));
            let corners = [
                Point::new(0.0, 0.0),
                Point::new(w, 0.0),
                Point::new(w, h),
                Point::new(0.0, h),
            ]
            .map(|p| screen(item_point(item, p)));
            polygon(image, &corners, ACCENT, None);
            let top = screen(item_point(item, Point::new(w / 2.0, 0.0)));
            let (sin, cos) = item.rotation.to_radians().sin_cos();
            let knob = Point::new(top.x + sin * ROTATE_OFFSET, top.y - cos * ROTATE_OFFSET);
            line(image, top, knob, ACCENT, 1.0, None);
            disc(image, knob, HANDLE_RADIUS, WHITE, ACCENT);
            for handle in HandlePos::ALL {
                square(
                    image,
                    screen(item_point(item, handle.at(w, h))),
                    HANDLE_RADIUS,
                    WHITE,
                    ACCENT,
                );
            }
            if let Some(feather) = item.feather {
                let f = item.frame();
                for edge in feathered_edges(item) {
                    let fw = feather.width;
                    let (a, b) = match edge {
                        Edge::Left => (Point::new(f.x + fw, f.y), Point::new(f.x + fw, f.bottom())),
                        Edge::Right => (
                            Point::new(f.right() - fw, f.y),
                            Point::new(f.right() - fw, f.bottom()),
                        ),
                        Edge::Top => (Point::new(f.x, f.y + fw), Point::new(f.right(), f.y + fw)),
                        Edge::Bottom => (
                            Point::new(f.x, f.bottom() - fw),
                            Point::new(f.right(), f.bottom() - fw),
                        ),
                    };
                    let selected = self.selection == Selection::FeatherEdge(item.id, edge);
                    line(
                        image,
                        screen(a),
                        screen(b),
                        FEATHER,
                        if selected { 2.0 } else { 1.0 },
                        Some(4.0),
                    );
                    square(
                        image,
                        screen(feather_handle(item, edge, fw)),
                        HANDLE_RADIUS,
                        WHITE,
                        FEATHER,
                    );
                }
            }
        }

        // Crop tool (CROP-001): dim the part being cut away.
        if let Some(crop) = self.crop
            && let Some(item) = state.item(crop.item)
        {
            let mapping = ItemMapping::new(*item);
            let b = item.bounds();
            let a = screen(Point::new(b.x, b.y));
            let z = screen(Point::new(b.right(), b.bottom()));
            let (w, h) = image.dimensions();
            let x0 = a.x.floor().max(0.0) as u32;
            let y0 = a.y.floor().max(0.0) as u32;
            let x1 = (z.x.ceil().max(0.0) as u32).min(w);
            let y1 = (z.y.ceil().max(0.0) as u32).min(h);
            for y in y0..y1 {
                for x in x0..x1 {
                    let p = view.to_canvas(Point::new(f64::from(x) + 0.5, f64::from(y) + 0.5));
                    if item.contains(p)
                        && !crop.rect.contains(mapping.canvas_to_source(p))
                        && let Some(px) = image.get_pixel_mut_checked(x, y)
                    {
                        px.0 = blend(px.0, [0, 0, 0, 0x90]);
                    }
                }
            }
            let r = crop.rect;
            let corners = [
                Point::new(r.x, r.y),
                Point::new(r.right(), r.y),
                Point::new(r.right(), r.bottom()),
                Point::new(r.x, r.bottom()),
            ]
            .map(|p| screen(mapping.source_to_canvas(p)));
            polygon(image, &corners, WHITE, None);
            for handle in HandlePos::ALL {
                let at = handle.at(r.width, r.height);
                let p = mapping.source_to_canvas(Point::new(r.x + at.x, r.y + at.y));
                square(image, screen(p), HANDLE_RADIUS, WHITE, BORDER);
            }
        }

        // Smear brush outline under the pointer (SMEAR-001).
        if let (Some(tool), Some(at)) = (self.smear_tool(), self.hover) {
            let r = tool.size * view.zoom / 2.0;
            let n = ((r * 0.8).ceil() as usize).clamped(16, 128);
            let ring: Vec<Point> = (0..n)
                .map(|i| {
                    let a = i as f64 / n as f64 * std::f64::consts::TAU;
                    Point::new(at.x + r * a.cos(), at.y + r * a.sin())
                })
                .collect();
            let outer: Vec<Point> = ring
                .iter()
                .map(|p| {
                    Point::new(
                        at.x + (p.x - at.x) * (r + 1.0) / r,
                        at.y + (p.y - at.y) * (r + 1.0) / r,
                    )
                })
                .collect();
            polygon(image, &outer, BORDER, None);
            polygon(image, &ring, WHITE, None);
        }

        // Snapping guides (PLACE-006).
        let (w, h) = image.dimensions();
        for guide in self.guides().to_vec() {
            let (a, b) = if guide.vertical {
                let x = screen(Point::new(guide.at, 0.0)).x;
                (Point::new(x, 0.0), Point::new(x, f64::from(h)))
            } else {
                let y = screen(Point::new(0.0, guide.at)).y;
                (Point::new(0.0, y), Point::new(f64::from(w), y))
            };
            line(image, a, b, GUIDE, 1.0, None);
        }
    }

    /// Labels for items whose source is missing (PROJ-005), at screen positions.
    pub fn labels(&mut self) -> Vec<(Point, String)> {
        let missing = self.missing_sources();
        let state = self.display_state().into_owned();
        let view = self.display_view();
        state
            .items
            .iter()
            .filter_map(|item| {
                let (_, name, _) = missing.iter().find(|(id, _, _)| *id == item.source)?;
                Some((
                    view.to_screen(item.bounds().center()),
                    format!("Missing: {name}"),
                ))
            })
            .collect()
    }
}

fn blend(dst: [u8; 4], src: [u8; 4]) -> [u8; 4] {
    let a = u32::from(src[3]);
    if a == 255 {
        return src;
    }
    if a == 0 {
        return dst;
    }
    let mix = |s: u8, d: u8| {
        let sum = u32::from(s)
            .saturating_mul(a)
            .saturating_add(u32::from(d).saturating_mul(255u32.saturating_sub(a)))
            .saturating_add(127);
        (sum / 255) as u8
    };
    [
        mix(src[0], dst[0]),
        mix(src[1], dst[1]),
        mix(src[2], dst[2]),
        0xff,
    ]
}

fn plot(image: &mut RgbaImage, x: f64, y: f64, color: [u8; 4]) {
    let (w, h) = image.dimensions();
    if x < 0.0 || y < 0.0 || x >= f64::from(w) || y >= f64::from(h) {
        return;
    }
    if let Some(p) = image.get_pixel_mut_checked(x as u32, y as u32) {
        p.0 = blend(p.0, color);
    }
}

fn line(image: &mut RgbaImage, a: Point, b: Point, color: [u8; 4], width: f64, dash: Option<f64>) {
    let length = a.distance(b);
    let steps = (length * 2.0).ceil().max(1.0) as usize;
    let half = (width / 2.0).max(0.5);
    let (dx, dy) = (
        (b.x - a.x) / length.max(1e-9),
        (b.y - a.y) / length.max(1e-9),
    );
    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        if let Some(d) = dash
            && ((t * length) / d) as i64 % 2 == 1
        {
            continue;
        }
        let (x, y) = (a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
        let mut o = -half + 0.5;
        while o < half {
            plot(image, x - dy * o, y + dx * o, color);
            o += 1.0;
        }
    }
}

fn polygon(image: &mut RgbaImage, points: &[Point], color: [u8; 4], dash: Option<f64>) {
    for (&a, &b) in points.iter().zip(points.iter().cycle().skip(1)) {
        line(image, a, b, color, 1.0, dash);
    }
}

fn square(image: &mut RgbaImage, c: Point, r: f64, fill: [u8; 4], border: [u8; 4]) {
    let (x0, y0) = ((c.x - r).round(), (c.y - r).round());
    let size = (2.0 * r).round() as i64;
    for dy in 0..=size {
        for dx in 0..=size {
            let edge = dx == 0 || dy == 0 || dx == size || dy == size;
            plot(
                image,
                x0 + dx as f64,
                y0 + dy as f64,
                if edge { border } else { fill },
            );
        }
    }
}

fn disc(image: &mut RgbaImage, c: Point, r: f64, fill: [u8; 4], border: [u8; 4]) {
    let ri = (r.ceil() as i64).saturating_add(1);
    for dy in ri.saturating_neg()..=ri {
        for dx in ri.saturating_neg()..=ri {
            let d = ((dx as f64).powi(2) + (dy as f64).powi(2)).sqrt();
            if d <= r + 0.5 {
                let color = if d >= r - 1.0 { border } else { fill };
                plot(image, c.x + dx as f64, c.y + dy as f64, color);
            }
        }
    }
}
