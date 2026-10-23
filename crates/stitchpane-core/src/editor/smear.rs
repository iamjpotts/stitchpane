//! The smear tool (SMEAR-001 to SMEAR-005). Strokes are drawn in canvas coordinates and
//! recorded in the item's source pixels, so they stay with the image.

use super::crop::ItemMapping;
use super::{Editor, Selection};
use crate::geometry::Point;
use crate::model::{ItemId, SmearKind, Step, Stroke};

/// Default strength (SMEAR-003).
pub const DEFAULT_STRENGTH: f64 = 0.5;

/// Brush settings, kept for the session (SMEAR-003).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct SmearSettings {
    pub(super) kind: SmearKind,
    /// Brush diameter in canvas pixels; `None` until the user sets one.
    pub(super) size: Option<f64>,
    pub(super) strength: f64,
}

impl Default for SmearSettings {
    fn default() -> Self {
        Self {
            kind: SmearKind::default(),
            size: None,
            strength: DEFAULT_STRENGTH,
        }
    }
}

/// The smear tool as the tool bar shows it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SmearTool {
    pub item: ItemId,
    pub kind: SmearKind,
    /// Brush diameter in canvas pixels.
    pub size: f64,
    /// `0.01..=1`.
    pub strength: f64,
}

impl Editor {
    /// Starts the smear tool on an item (SMEAR-001).
    pub(super) fn start_smear(&mut self, item: ItemId) -> bool {
        if self.project.state().item(item).is_none() {
            return false;
        }
        self.crop = None;
        self.smear = Some(item);
        self.selection = Selection::Item(item);
        true
    }

    /// The active smear tool and its brush, if any.
    pub fn smear_tool(&self) -> Option<SmearTool> {
        let id = self.smear?;
        let item = self.project.state().item(id)?;
        let default = (f64::from(item.width.min(item.height)) * 0.05)
            .max(1.0)
            .round();
        Some(SmearTool {
            item: id,
            kind: self.smear_settings.kind,
            size: self.smear_settings.size.unwrap_or(default),
            strength: self.smear_settings.strength,
        })
    }

    pub fn set_smear_kind(&mut self, kind: SmearKind) {
        self.smear_settings.kind = kind;
    }

    /// Sets the brush diameter in canvas pixels.
    pub fn set_smear_size(&mut self, size: f64) {
        if size.is_finite() && size >= 1.0 {
            self.smear_settings.size = Some(size);
        }
    }

    /// Sets the strength, `0.01..=1`.
    pub fn set_smear_strength(&mut self, strength: f64) {
        if strength.is_finite() {
            self.smear_settings.strength = strength.clamp(0.01, 1.0);
        }
    }

    /// The stroke for canvas points drawn with the current brush, in the item's source
    /// pixels (SMEAR-005).
    pub(super) fn stroke_from(&self, points: &[Point]) -> Option<(ItemId, Stroke)> {
        let tool = self.smear_tool()?;
        let item = *self.project.state().item(tool.item)?;
        let mapping = ItemMapping::new(item);
        let scale = (f64::from(item.width) / f64::from(item.crop.width)
            + f64::from(item.height) / f64::from(item.crop.height))
            / 2.0;
        let points = points
            .iter()
            .map(|&p| {
                // Hundredths of a pixel keep project files short.
                let s = mapping.canvas_to_source(p);
                [(s.x * 100.0).round() / 100.0, (s.y * 100.0).round() / 100.0]
            })
            .collect();
        Some((
            tool.item,
            Stroke {
                kind: tool.kind,
                size: tool.size / scale,
                strength: tool.strength,
                points,
            },
        ))
    }

    /// Records a finished stroke (SMEAR-004).
    pub(super) fn finish_stroke(&mut self, points: &[Point]) -> bool {
        match self.stroke_from(points) {
            Some((item, stroke)) => self.record(Step::Smear { item, stroke }),
            None => false,
        }
    }
}
