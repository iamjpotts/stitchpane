//! The crop tool (CROP-001 to CROP-004, PLACE-004). The crop rectangle is kept in source
//! pixels and limited to the part of the source the item currently shows.

use super::Editor;
use crate::geometry::{PixelRect, Point, Rect};
use crate::model::{Item, ItemId, Step};

/// Aspect ratio choices (CROP-003).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AspectChoice {
    Free,
    Original,
    Square,
    FourThree,
    ThreeTwo,
    SixteenNine,
    Custom(u32, u32),
}

impl AspectChoice {
    pub const PRESETS: [AspectChoice; 6] = [
        AspectChoice::Free,
        AspectChoice::Original,
        AspectChoice::Square,
        AspectChoice::FourThree,
        AspectChoice::ThreeTwo,
        AspectChoice::SixteenNine,
    ];

    pub fn label(self) -> String {
        match self {
            AspectChoice::Free => "Free".to_owned(),
            AspectChoice::Original => "Original".to_owned(),
            AspectChoice::Square => "1:1".to_owned(),
            AspectChoice::FourThree => "4:3".to_owned(),
            AspectChoice::ThreeTwo => "3:2".to_owned(),
            AspectChoice::SixteenNine => "16:9".to_owned(),
            AspectChoice::Custom(w, h) => format!("{w}:{h}"),
        }
    }

    /// Width divided by height, or `None` for free.
    pub fn ratio(self, original: (u32, u32)) -> Option<f64> {
        let r = |w: u32, h: u32| (h > 0).then(|| f64::from(w) / f64::from(h));
        match self {
            AspectChoice::Free => None,
            AspectChoice::Original => r(original.0, original.1),
            AspectChoice::Square => Some(1.0),
            AspectChoice::FourThree => Some(4.0 / 3.0),
            AspectChoice::ThreeTwo => Some(1.5),
            AspectChoice::SixteenNine => Some(16.0 / 9.0),
            AspectChoice::Custom(w, h) => r(w, h),
        }
    }
}

/// State of the crop tool while it is active.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CropTool {
    pub item: ItemId,
    /// The rectangle, in source pixels.
    pub rect: Rect,
    /// The item's crop when the tool started; the rectangle stays inside it.
    pub limits: PixelRect,
    pub aspect: AspectChoice,
    /// Upright size of the whole source, for the "original" ratio.
    pub source_size: (u32, u32),
}

impl CropTool {
    fn new(item: &Item) -> Self {
        let c = item.crop;
        Self {
            item: item.id,
            rect: Rect::new(
                f64::from(c.left),
                f64::from(c.top),
                f64::from(c.width),
                f64::from(c.height),
            ),
            limits: c,
            aspect: AspectChoice::Free,
            source_size: (item.source_width, item.source_height),
        }
    }

    pub fn ratio(&self) -> Option<f64> {
        self.aspect.ratio(self.source_size)
    }

    fn limit_rect(&self) -> Rect {
        let l = self.limits;
        Rect::new(
            f64::from(l.left),
            f64::from(l.top),
            f64::from(l.width),
            f64::from(l.height),
        )
    }

    /// The rectangle rounded to whole pixels inside the limits (CROP-002 fields).
    pub fn pixel_rect(&self) -> PixelRect {
        let lim = self.limit_rect();
        let left = self.rect.x.round().clamp(lim.x, lim.right() - 1.0);
        let top = self.rect.y.round().clamp(lim.y, lim.bottom() - 1.0);
        let right = self.rect.right().round().clamp(left + 1.0, lim.right());
        let bottom = self.rect.bottom().round().clamp(top + 1.0, lim.bottom());
        PixelRect::new(
            left as u32,
            top as u32,
            (right - left) as u32,
            (bottom - top) as u32,
        )
    }

    /// Moves the rectangle, keeping it inside the limits.
    pub(super) fn translate(&mut self, dx: f64, dy: f64) {
        let lim = self.limit_rect();
        self.rect.x = (self.rect.x + dx).clamp(lim.x, lim.right() - self.rect.width);
        self.rect.y = (self.rect.y + dy).clamp(lim.y, lim.bottom() - self.rect.height);
    }

    /// Sets the rectangle from edges, clamped to the limits and at least 1 pixel.
    pub(super) fn set_edges(&mut self, left: f64, top: f64, right: f64, bottom: f64) {
        let lim = self.limit_rect();
        let left = left.clamp(lim.x, lim.right() - 1.0);
        let top = top.clamp(lim.y, lim.bottom() - 1.0);
        let right = right.clamp(left + 1.0, lim.right());
        let bottom = bottom.clamp(top + 1.0, lim.bottom());
        self.rect = Rect::new(left, top, right - left, bottom - top);
    }

    /// Shrinks the rectangle about its center to the chosen ratio (CROP-003).
    pub(super) fn apply_ratio(&mut self) {
        let Some(ratio) = self.ratio() else { return };
        let c = self.rect.center();
        let (mut w, mut h) = (self.rect.width, self.rect.height);
        if w / h > ratio {
            w = h * ratio;
        } else {
            h = w / ratio;
        }
        let lim = self.limit_rect();
        w = w.min(lim.width);
        h = h.min(lim.height);
        let x = (c.x - w / 2.0).clamp(lim.x, lim.right() - w);
        let y = (c.y - h / 2.0).clamp(lim.y, lim.bottom() - h);
        self.rect = Rect::new(x, y, w, h);
    }
}

/// Converts between an item's source pixels and canvas points.
pub(super) struct ItemMapping {
    item: Item,
}

impl ItemMapping {
    pub(super) fn new(item: Item) -> Self {
        Self { item }
    }

    fn scale(&self) -> (f64, f64) {
        (
            f64::from(self.item.width) / f64::from(self.item.crop.width),
            f64::from(self.item.height) / f64::from(self.item.crop.height),
        )
    }

    /// Source pixel → item-local point.
    pub(super) fn source_to_local(&self, p: Point) -> Point {
        let (sx, sy) = self.scale();
        Point::new(
            (p.x - f64::from(self.item.crop.left)) * sx,
            (p.y - f64::from(self.item.crop.top)) * sy,
        )
    }

    /// Source pixel → canvas point.
    pub(super) fn source_to_canvas(&self, p: Point) -> Point {
        let local = self.source_to_local(p);
        let frame = self.item.frame();
        Point::new(frame.x + local.x, frame.y + local.y)
            .rotated_about(frame.center(), self.item.rotation)
    }

    /// Canvas point → source pixel.
    pub(super) fn canvas_to_source(&self, p: Point) -> Point {
        let local = self.item.to_local(p);
        let (sx, sy) = self.scale();
        Point::new(
            local.x / sx + f64::from(self.item.crop.left),
            local.y / sy + f64::from(self.item.crop.top),
        )
    }
}

impl Editor {
    /// Activates the crop tool on an item (CROP-001, PLACE-004).
    pub(super) fn start_crop(&mut self, item: ItemId) -> bool {
        let Some(item) = self.project.state().item(item).copied() else {
            return false;
        };
        self.crop = Some(CropTool::new(&item));
        self.selection = super::Selection::Item(item.id);
        true
    }

    /// Sets the crop rectangle from the numeric fields (CROP-002).
    pub fn set_crop_rect(&mut self, rect: PixelRect) {
        if let Some(crop) = &mut self.crop {
            crop.set_edges(
                f64::from(rect.left),
                f64::from(rect.top),
                f64::from(rect.right()),
                f64::from(rect.bottom()),
            );
        }
    }

    /// Chooses an aspect ratio (CROP-003).
    pub fn set_crop_aspect(&mut self, aspect: AspectChoice) {
        if let Some(crop) = &mut self.crop {
            crop.aspect = aspect;
            crop.apply_ratio();
        }
    }

    /// Records the crop (CROP-004). The kept region stays where it was on the canvas.
    pub(super) fn apply_crop(&mut self) -> bool {
        let Some(tool) = self.crop.take() else {
            return false;
        };
        let Some(item) = self.project.state().item(tool.item).copied() else {
            return false;
        };
        let crop = tool.pixel_rect();
        if crop == item.crop {
            return true;
        }
        let mapping = ItemMapping::new(item);
        let (sx, sy) = mapping.scale();
        let width = ((f64::from(crop.width) * sx).round() as u32).max(1);
        let height = ((f64::from(crop.height) * sy).round() as u32).max(1);
        let center = mapping.source_to_canvas(Point::new(
            f64::from(crop.left) + f64::from(crop.width) / 2.0,
            f64::from(crop.top) + f64::from(crop.height) / 2.0,
        ));
        self.record(Step::CropItem {
            item: item.id,
            crop,
            x: (center.x - f64::from(width) / 2.0).round() as i32,
            y: (center.y - f64::from(height) / 2.0).round() as i32,
            width,
            height,
        })
    }
}
