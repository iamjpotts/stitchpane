//! The dodge and burn tool (DODGE-001 to DODGE-004). Strokes are drawn and recorded as
//! smear strokes are (see `smear.rs`).

use super::{Editor, Selection};
use crate::model::{ItemId, StrokeKind, ToneRange};
use crate::panic_free::Clamped;

/// Default strength (DODGE-003).
pub const DEFAULT_STRENGTH: f64 = 0.5;

/// Brush settings, kept for the session (DODGE-003).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct DodgeSettings {
    pub(super) kind: StrokeKind,
    pub(super) range: ToneRange,
    /// Brush diameter in canvas pixels; `None` until the user sets one.
    pub(super) size: Option<f64>,
    pub(super) strength: f64,
}

impl Default for DodgeSettings {
    fn default() -> Self {
        Self {
            kind: StrokeKind::Dodge,
            range: ToneRange::default(),
            size: None,
            strength: DEFAULT_STRENGTH,
        }
    }
}

/// The dodge and burn tool as the tool bar shows it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DodgeTool {
    pub item: ItemId,
    /// [`StrokeKind::Dodge`] or [`StrokeKind::Burn`].
    pub kind: StrokeKind,
    pub range: ToneRange,
    /// Brush diameter in canvas pixels.
    pub size: f64,
    /// `0.01..=1`.
    pub strength: f64,
}

impl Editor {
    /// Starts the dodge and burn tool on an item (DODGE-001).
    pub(super) fn start_dodge(&mut self, item: ItemId) -> bool {
        if self.project.state().item(item).is_none() {
            return false;
        }
        self.leave_tools();
        self.dodge = Some(item);
        self.selection = Selection::Item(item);
        true
    }

    /// The active dodge and burn tool and its brush, if any.
    pub fn dodge_tool(&self) -> Option<DodgeTool> {
        let id = self.dodge?;
        let item = self.project.state().item(id)?;
        let default = (f64::from(item.width.min(item.height)) * 0.05)
            .max(1.0)
            .round();
        let s = self.dodge_settings;
        Some(DodgeTool {
            item: id,
            kind: s.kind,
            range: s.range,
            size: s.size.unwrap_or(default),
            strength: s.strength,
        })
    }

    /// Sets dodge or burn; other kinds are ignored.
    pub fn set_dodge_kind(&mut self, kind: StrokeKind) {
        if !kind.is_smear() {
            self.dodge_settings.kind = kind;
        }
    }

    pub fn set_dodge_range(&mut self, range: ToneRange) {
        self.dodge_settings.range = range;
    }

    /// Sets the brush diameter in canvas pixels.
    pub fn set_dodge_size(&mut self, size: f64) {
        if size.is_finite() && size >= 1.0 {
            self.dodge_settings.size = Some(size);
        }
    }

    /// Sets the strength, `0.01..=1`.
    pub fn set_dodge_strength(&mut self, strength: f64) {
        if strength.is_finite() {
            self.dodge_settings.strength = strength.clamped(0.01, 1.0);
        }
    }
}
