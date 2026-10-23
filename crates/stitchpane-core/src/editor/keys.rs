//! Keyboard shortcuts and fine adjustment with the keyboard (UI-006, shortcut table in
//! `docs/spec/ui/workflows.md`).

use super::{AdjustTarget, Command, Edge, Editor, Selection, round_tenth};
use crate::model::{ProjectState, Step, TransitionKind};

/// Keys the editor understands. UIs translate their key events to these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Left,
    Right,
    Up,
    Down,
    BracketLeft,
    BracketRight,
    Escape,
    Enter,
    Delete,
    Backspace,
    Plus,
    Minus,
    /// Any other character key, as typed without Shift.
    Char(char),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

impl Modifiers {
    pub const NONE: Modifiers = Modifiers {
        shift: false,
        ctrl: false,
        alt: false,
    };
}

/// Result of a key press.
#[derive(Clone, Debug, PartialEq)]
pub enum KeyOutcome {
    Unhandled,
    Handled,
    /// A command that needs a dialog from the UI.
    Command(Command),
}

impl Editor {
    pub fn key_press(&mut self, key: Key, mods: Modifiers) -> KeyOutcome {
        let key = match key {
            Key::Char(c) => Key::Char(c.to_ascii_lowercase()),
            other => other,
        };
        if mods.ctrl {
            let command = match key {
                Key::Char('o') => Command::Open,
                Key::Char('s') if mods.shift => Command::SaveAs,
                Key::Char('s') => Command::Save,
                Key::Char('e') => Command::Export,
                Key::Char('z') if mods.shift => Command::Redo,
                Key::Char('z') => Command::Undo,
                Key::Char('y') => Command::Redo,
                Key::Plus | Key::Char('=') => Command::ZoomIn,
                Key::Minus => Command::ZoomOut,
                Key::Char('0') => Command::ZoomFit,
                Key::Char('1') => Command::Zoom100,
                Key::Left | Key::Right | Key::Up | Key::Down => {
                    return self.arrow(key, mods);
                }
                _ => return KeyOutcome::Unhandled,
            };
            return self.run(command);
        }
        match key {
            Key::Left | Key::Right | Key::Up | Key::Down => self.arrow(key, mods),
            Key::BracketLeft | Key::BracketRight => {
                let Some(item) = self.selection.item() else {
                    return KeyOutcome::Unhandled;
                };
                let amount = if mods.shift { 1.0 } else { 0.1 };
                let delta = if key == Key::BracketLeft {
                    -amount
                } else {
                    amount
                };
                self.adjust(AdjustTarget::Rotate(item), |state| {
                    let current = state.item(item)?;
                    Some(Step::RotateItem {
                        item,
                        degrees: round_tenth(current.rotation + delta).rem_euclid(360.0),
                    })
                });
                KeyOutcome::Handled
            }
            Key::Char('c') => {
                let target = self.selection.item().or_else(|| {
                    let items = &self.project.state().items;
                    match items.as_slice() {
                        [only] => Some(only.id),
                        _ => None,
                    }
                });
                match target {
                    Some(item) => self.run(Command::Crop(item)),
                    None => KeyOutcome::Unhandled,
                }
            }
            Key::Char('s') => match self.selection.item() {
                Some(item) => self.run(Command::Smear(item)),
                None => KeyOutcome::Unhandled,
            },
            Key::Enter | Key::Escape if self.smear.is_some() => self.run(Command::EndSmear),
            Key::Delete | Key::Backspace => match self.selection.item() {
                Some(item) => self.run(Command::RemoveItem(item)),
                None => KeyOutcome::Unhandled,
            },
            Key::Enter if self.crop.is_some() => self.run(Command::ApplyCrop),
            Key::Escape if self.crop.is_some() => self.run(Command::CancelCrop),
            Key::Escape if self.selection != Selection::None => {
                self.select(Selection::None);
                KeyOutcome::Handled
            }
            _ => KeyOutcome::Unhandled,
        }
    }

    fn run(&mut self, command: Command) -> KeyOutcome {
        if command.needs_ui() {
            KeyOutcome::Command(command)
        } else {
            self.execute(command);
            KeyOutcome::Handled
        }
    }

    /// Arrow keys: move, or with Ctrl resize, whatever is selected (UI-006).
    fn arrow(&mut self, key: super::Key, mods: Modifiers) -> KeyOutcome {
        let amount: i32 = if mods.shift { 10 } else { 1 };
        let (dx, dy) = match key {
            Key::Left => (amount.saturating_neg(), 0),
            Key::Right => (amount, 0),
            Key::Up => (0, amount.saturating_neg()),
            _ => (0, amount),
        };
        if let Some(crop) = &mut self.crop {
            let (fx, fy) = (f64::from(dx), f64::from(dy));
            if mods.ctrl {
                let r = crop.rect;
                crop.set_edges(r.x, r.y, r.right() + fx, r.bottom() + fy);
            } else {
                crop.translate(fx, fy);
            }
            return KeyOutcome::Handled;
        }
        match self.selection {
            Selection::Item(item) if mods.ctrl => {
                self.adjust(AdjustTarget::Resize(item), |state| {
                    let i = state.item(item)?;
                    Some(Step::ResizeItem {
                        item,
                        x: i.x,
                        y: i.y,
                        width: grow(i.width, dx),
                        height: grow(i.height, dy),
                        filter: i.filter,
                    })
                });
            }
            Selection::Item(item) => {
                self.adjust(AdjustTarget::Move(item), |state| {
                    let i = state.item(item)?;
                    Some(Step::MoveItem {
                        item,
                        x: i.x.saturating_add(dx),
                        y: i.y.saturating_add(dy),
                    })
                });
            }
            Selection::Transition(id) => {
                self.adjust(AdjustTarget::Seam(id), |state| {
                    let t = state.transition(id)?;
                    let TransitionKind::GradientSeam {
                        orientation,
                        offset,
                        width,
                        curve,
                    } = t.kind
                    else {
                        return None;
                    };
                    let delta = match orientation {
                        crate::model::Orientation::Horizontal => dx,
                        crate::model::Orientation::Vertical => dy,
                    };
                    (delta != 0).then_some(Step::EditTransition {
                        id,
                        kind: TransitionKind::GradientSeam {
                            orientation,
                            offset: offset + f64::from(delta),
                            width,
                            curve,
                        },
                    })
                });
            }
            Selection::FeatherEdge(item, edge) => {
                self.adjust(AdjustTarget::Feather(item), |state| {
                    let mut feather = state.item(item)?.feather?;
                    let delta = match edge {
                        Edge::Left => dx,
                        Edge::Right => dx.saturating_neg(),
                        Edge::Top => dy,
                        Edge::Bottom => dy.saturating_neg(),
                    };
                    feather.width = (feather.width + f64::from(delta)).max(1.0);
                    Some(Step::SetFeather {
                        item,
                        feather: Some(feather),
                    })
                });
            }
            Selection::None => return KeyOutcome::Unhandled,
        }
        KeyOutcome::Handled
    }

    /// Updates the pending step of a keyboard run; a different target ends the run.
    fn adjust(&mut self, target: AdjustTarget, make: impl FnOnce(&ProjectState) -> Option<Step>) {
        if self.pending.as_ref().is_some_and(|(t, _)| *t != target) {
            self.commit_pending();
        }
        let state = self.display_state().into_owned();
        let Some(step) = make(&state) else { return };
        let mut check = self.project.state().clone();
        // a step the project rejects is not offered as pending
        if let Ok(()) = check.apply(&step) {
            self.pending = Some((target, step));
        }
    }
}

/// `size` grown by `delta` pixels, at least 1.
fn grow(size: u32, delta: i32) -> u32 {
    size.saturating_add_signed(delta).max(1)
}
