//! The light tool (LIGHT-001 to LIGHT-006): the item's light settings, edited with a live
//! preview and recorded as one step when applied.

use super::{Editor, Selection};
use crate::model::{ItemId, Light, Step};

/// The light tool as the tool bar shows it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightTool {
    pub item: ItemId,
    pub light: Light,
}

impl Editor {
    /// Starts the light tool on an item with its current settings (LIGHT-001, LIGHT-006).
    pub(super) fn start_light(&mut self, item: ItemId) -> bool {
        let Some(current) = self.project.state().item(item) else {
            return false;
        };
        let light = current.light.unwrap_or_else(|| self.neutral_light(item));
        self.leave_tools();
        self.light = Some(LightTool { item, light });
        self.selection = Selection::Item(item);
        true
    }

    /// The active light tool and its settings, if any.
    pub fn light_tool(&self) -> Option<LightTool> {
        self.light
    }

    /// Changes the settings shown while the tool is active. Out-of-range settings are
    /// ignored, reporting why as the message.
    pub fn set_light(&mut self, light: Light) {
        let Some(tool) = &mut self.light else { return };
        match light.check() {
            Ok(()) => tool.light = light,
            Err(e) => self.message = Some(e.to_string()),
        }
    }

    /// Neutral settings with the item's default radius (LIGHT-001 Reset, LIGHT-003).
    fn neutral_light(&self, item: ItemId) -> Light {
        let radius = self.project.state().item(item).map_or(1.0, |i| {
            Light::default_radius(i.source_width, i.source_height)
        });
        Light::neutral(radius)
    }

    pub(super) fn reset_light(&mut self) -> bool {
        let Some(tool) = self.light else { return false };
        let neutral = self.neutral_light(tool.item);
        self.light = Some(LightTool {
            light: neutral,
            ..tool
        });
        true
    }

    /// Records the settings as one step, unless they are what the item already has
    /// (LIGHT-001, LIGHT-006).
    pub(super) fn apply_light(&mut self) -> bool {
        let Some(tool) = self.light.take() else {
            return false;
        };
        let current = self.project.state().item(tool.item).and_then(|i| i.light);
        let unchanged = match current {
            Some(current) => current == tool.light,
            None => tool.light.is_neutral(),
        };
        if unchanged {
            return true;
        }
        self.record(Step::SetLight {
            item: tool.item,
            light: tool.light,
        })
    }
}
