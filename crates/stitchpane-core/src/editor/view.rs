//! Mapping between canvas coordinates and the screen (UI-003).

use crate::geometry::{Point, Rect};
use crate::panic_free::Clamped;

pub const MIN_ZOOM: f64 = 0.02;
pub const MAX_ZOOM: f64 = 32.0;
/// Zoom factor of one zoom-in or zoom-out step.
pub const ZOOM_STEP: f64 = 1.25;
/// Space left around the canvas when zoomed to fit, in screen pixels.
const FIT_MARGIN: f64 = 24.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    /// Screen pixels per canvas pixel.
    pub zoom: f64,
    /// Canvas point shown at the viewport's top-left corner.
    pub pan: Point,
    /// Viewport size in screen pixels.
    pub viewport: (f64, f64),
    /// While true, the view follows canvas size changes to keep the canvas fitted.
    pub fit: bool,
}

impl Default for View {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: Point::default(),
            viewport: (800.0, 600.0),
            fit: true,
        }
    }
}

impl View {
    pub fn to_screen(&self, p: Point) -> Point {
        Point::new(
            (p.x - self.pan.x) * self.zoom,
            (p.y - self.pan.y) * self.zoom,
        )
    }

    pub fn to_canvas(&self, s: Point) -> Point {
        Point::new(s.x / self.zoom + self.pan.x, s.y / self.zoom + self.pan.y)
    }

    /// Canvas area visible in the viewport.
    pub fn visible(&self) -> Rect {
        Rect::new(
            self.pan.x,
            self.pan.y,
            self.viewport.0 / self.zoom,
            self.viewport.1 / self.zoom,
        )
    }

    /// Zooms so the canvas fits the viewport, centered (Ctrl+0).
    pub fn fit_to(&mut self, canvas: Rect) {
        let (vw, vh) = self.viewport;
        if canvas.is_empty() {
            self.zoom = 1.0;
            self.pan = Point::new(canvas.x - vw / 2.0, canvas.y - vh / 2.0);
        } else {
            let zoom = ((vw - 2.0 * FIT_MARGIN) / canvas.width)
                .min((vh - 2.0 * FIT_MARGIN) / canvas.height)
                .clamped(MIN_ZOOM, MAX_ZOOM);
            self.zoom = zoom;
            self.center_on(canvas);
        }
        self.fit = true;
    }

    fn center_on(&mut self, canvas: Rect) {
        let c = canvas.center();
        self.pan = Point::new(
            c.x - self.viewport.0 / 2.0 / self.zoom,
            c.y - self.viewport.1 / 2.0 / self.zoom,
        );
    }

    /// Sets the zoom, keeping the canvas point under `about` (screen) in place.
    pub fn set_zoom(&mut self, zoom: f64, about: Point) {
        let anchor = self.to_canvas(about);
        self.zoom = zoom.clamped(MIN_ZOOM, MAX_ZOOM);
        self.pan = Point::new(
            anchor.x - about.x / self.zoom,
            anchor.y - about.y / self.zoom,
        );
        self.fit = false;
    }

    pub fn zoom_by(&mut self, factor: f64, about: Point) {
        self.set_zoom(self.zoom * factor, about);
    }

    pub fn viewport_center(&self) -> Point {
        Point::new(self.viewport.0 / 2.0, self.viewport.1 / 2.0)
    }

    /// Scrolls by a number of screen pixels.
    pub fn scroll(&mut self, dx: f64, dy: f64) {
        self.pan.x += dx / self.zoom;
        self.pan.y += dy / self.zoom;
        self.fit = false;
    }
}
