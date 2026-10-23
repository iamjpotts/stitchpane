//! Points and rectangles in canvas coordinates (pixels, y down).

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn distance(self, other: Point) -> f64 {
        (self.x - other.x).hypot(self.y - other.y)
    }

    /// Rotates this point about `center` by `degrees` (clockwise on screen, y down).
    pub fn rotated_about(self, center: Point, degrees: f64) -> Point {
        let (sin, cos) = degrees.to_radians().sin_cos();
        let dx = self.x - center.x;
        let dy = self.y - center.y;
        Point::new(
            center.x + dx * cos - dy * sin,
            center.y + dx * sin + dy * cos,
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn right(&self) -> f64 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f64 {
        self.y + self.height
    }

    pub fn center(&self) -> Point {
        Point::new(self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x < self.right() && p.y >= self.y && p.y < self.bottom()
    }

    pub fn is_empty(&self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }

    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        (right > x && bottom > y).then(|| Rect::new(x, y, right - x, bottom - y))
    }

    pub fn union(&self, other: &Rect) -> Rect {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Rect::new(
            x,
            y,
            self.right().max(other.right()) - x,
            self.bottom().max(other.bottom()) - y,
        )
    }

    /// The smallest integer-aligned rectangle containing this one.
    pub fn round_out(&self) -> Rect {
        let x = self.x.floor();
        let y = self.y.floor();
        Rect::new(x, y, self.right().ceil() - x, self.bottom().ceil() - y)
    }
}

/// An integer rectangle in image pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PixelRect {
    pub left: u32,
    pub top: u32,
    pub width: u32,
    pub height: u32,
}

impl PixelRect {
    pub const fn new(left: u32, top: u32, width: u32, height: u32) -> Self {
        Self {
            left,
            top,
            width,
            height,
        }
    }

    pub fn right(&self) -> u32 {
        self.left + self.width
    }

    pub fn bottom(&self) -> u32 {
        self.top + self.height
    }
}

/// Bounding box of a `width` × `height` box centered on `center` and rotated by `degrees`.
pub fn rotated_bounds(center: Point, width: f64, height: f64, degrees: f64) -> Rect {
    let (sin, cos) = degrees.to_radians().sin_cos();
    let (sin, cos) = (sin.abs(), cos.abs());
    // Quarter turns land exactly on integers instead of drifting by rounding error.
    let (sin, cos) = (snap_unit(sin), snap_unit(cos));
    let w = width * cos + height * sin;
    let h = width * sin + height * cos;
    Rect::new(center.x - w / 2.0, center.y - h / 2.0, w, h)
}

fn snap_unit(v: f64) -> f64 {
    if v < 1e-12 {
        0.0
    } else if (1.0 - v) < 1e-12 {
        1.0
    } else {
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intersection_and_union() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(5.0, 5.0, 10.0, 10.0);
        assert_eq!(a.intersection(&b), Some(Rect::new(5.0, 5.0, 5.0, 5.0)));
        assert_eq!(a.union(&b), Rect::new(0.0, 0.0, 15.0, 15.0));
        assert_eq!(a.intersection(&Rect::new(20.0, 0.0, 1.0, 1.0)), None);
    }

    #[test]
    fn quarter_turn_bounds_swap_sides() {
        let r = rotated_bounds(Point::new(50.0, 20.0), 100.0, 40.0, 90.0);
        assert_eq!(r, Rect::new(30.0, -30.0, 40.0, 100.0));
    }

    #[test]
    fn rotate_point() {
        let p = Point::new(1.0, 0.0).rotated_about(Point::default(), 90.0);
        assert!((p.x).abs() < 1e-9 && (p.y - 1.0).abs() < 1e-9);
    }
}
