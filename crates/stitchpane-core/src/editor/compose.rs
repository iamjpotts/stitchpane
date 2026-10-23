//! Creating compositions and adding images (CANVAS-001, PLACE-001), and size arithmetic
//! for the resolution dialogs (RES-001, RES-004).

use std::path::{Path, PathBuf};

use super::{Editor, Selection};
use crate::geometry::Point;
use crate::model::{Canvas, CanvasSize, Step};
use crate::project::Project;

/// Fixed canvas sizes offered when creating a composition (CANVAS-001).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    FullHd,
    UltraHd,
    A4,
    Letter,
}

impl Preset {
    pub const ALL: [Preset; 4] = [Preset::FullHd, Preset::UltraHd, Preset::A4, Preset::Letter];

    pub fn label(self) -> &'static str {
        match self {
            Preset::FullHd => "1920 × 1080",
            Preset::UltraHd => "3840 × 2160",
            Preset::A4 => "A4 at 300 DPI",
            Preset::Letter => "US Letter at 300 DPI",
        }
    }

    pub fn size(self) -> (u32, u32) {
        match self {
            Preset::FullHd => (1920, 1080),
            Preset::UltraHd => (3840, 2160),
            Preset::A4 => (2480, 3508),
            Preset::Letter => (2550, 3300),
        }
    }

    pub fn dpi(self) -> Option<f64> {
        match self {
            Preset::A4 | Preset::Letter => Some(300.0),
            _ => None,
        }
    }
}

/// Canvas size choices when creating a composition (CANVAS-001).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CanvasChoice {
    /// Widths summed, height of the tallest (default).
    SideBySide,
    /// Heights summed, width of the widest.
    Stacked,
    Largest,
    Preset(Preset),
    Custom {
        width: u32,
        height: u32,
        dpi: Option<f64>,
    },
}

impl CanvasChoice {
    pub fn label(&self) -> String {
        match self {
            CanvasChoice::SideBySide => "Side by side".to_owned(),
            CanvasChoice::Stacked => "Stacked".to_owned(),
            CanvasChoice::Largest => "Size of the largest image".to_owned(),
            CanvasChoice::Preset(p) => p.label().to_owned(),
            CanvasChoice::Custom { .. } => "Custom".to_owned(),
        }
    }

    /// The canvas and each image's top-left position for images of the given sizes.
    pub fn layout(&self, sizes: &[(u32, u32)]) -> (Canvas, Vec<(i32, i32)>) {
        let max_w = sizes.iter().map(|s| s.0).max().unwrap_or(1);
        let max_h = sizes.iter().map(|s| s.1).max().unwrap_or(1);
        let fixed = |width: u32, height: u32, dpi: Option<f64>| Canvas {
            size: CanvasSize::Fixed {
                width: width.max(1),
                height: height.max(1),
            },
            dpi,
        };
        let centered = |cw: u32, ch: u32| -> Vec<(i32, i32)> {
            sizes
                .iter()
                .map(|&(w, h)| (center(cw, w), center(ch, h)))
                .collect()
        };
        match *self {
            CanvasChoice::SideBySide => {
                let mut x = 0i32;
                let positions = sizes
                    .iter()
                    .map(|&(w, h)| {
                        let p = (x, center(max_h, h));
                        x += w as i32;
                        p
                    })
                    .collect();
                (
                    fixed(sizes.iter().map(|s| s.0).sum(), max_h, None),
                    positions,
                )
            }
            CanvasChoice::Stacked => {
                let mut y = 0i32;
                let positions = sizes
                    .iter()
                    .map(|&(w, h)| {
                        let p = (center(max_w, w), y);
                        y += h as i32;
                        p
                    })
                    .collect();
                (
                    fixed(max_w, sizes.iter().map(|s| s.1).sum(), None),
                    positions,
                )
            }
            CanvasChoice::Largest => {
                let &(w, h) = sizes
                    .iter()
                    .max_by_key(|(w, h)| u64::from(*w) * u64::from(*h))
                    .unwrap_or(&(1, 1));
                (fixed(w, h, None), centered(w, h))
            }
            CanvasChoice::Preset(preset) => {
                let (w, h) = preset.size();
                (fixed(w, h, preset.dpi()), centered(w, h))
            }
            CanvasChoice::Custom { width, height, dpi } => {
                (fixed(width, height, dpi), centered(width, height))
            }
        }
    }
}

fn center(outer: u32, inner: u32) -> i32 {
    ((i64::from(outer) - i64::from(inner)) / 2) as i32
}

/// Units for print sizes (RES-004).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrintUnit {
    Inches,
    Centimeters,
}

impl PrintUnit {
    pub fn label(self) -> &'static str {
        match self {
            PrintUnit::Inches => "in",
            PrintUnit::Centimeters => "cm",
        }
    }
}

/// Pixels needed for a print size at a DPI (RES-004).
pub fn pixels_for_print_size(size: f64, unit: PrintUnit, dpi: f64) -> u32 {
    let inches = match unit {
        PrintUnit::Inches => size,
        PrintUnit::Centimeters => size / 2.54,
    };
    ((inches * dpi).round() as u32).max(1)
}

/// The other dimension when resizing with the aspect ratio locked (RES-001).
pub fn proportional(original: u32, original_other: u32, new: u32) -> u32 {
    ((f64::from(new) * f64::from(original_other) / f64::from(original)).round() as u32).max(1)
}

impl Editor {
    /// Reads an image's upright size, registering nothing.
    fn image_size(&mut self, path: &Path) -> Result<(u32, u32), String> {
        let source = self
            .renderer
            .source(path)
            .map_err(|problem| match problem {
                crate::render::SourceProblem::Missing => {
                    format!("{} does not exist", path.display())
                }
                crate::render::SourceProblem::Unreadable(e) => format!("{}: {e}", path.display()),
            })?;
        Ok(source.pixels.dimensions())
    }

    /// Starts a new project from images. `None` makes a single-image project whose canvas
    /// tracks the image (UI-005); otherwise the canvas is chosen as in CANVAS-001.
    pub fn new_composition(
        &mut self,
        paths: &[PathBuf],
        choice: Option<CanvasChoice>,
    ) -> Result<(), String> {
        let paths: Vec<PathBuf> = paths
            .iter()
            .map(|p| std::path::absolute(p).map_err(|e| e.to_string()))
            .collect::<Result<_, _>>()?;
        let sizes = paths
            .iter()
            .map(|p| self.image_size(p))
            .collect::<Result<Vec<_>, _>>()?;
        let (canvas, positions) = match choice {
            Some(choice) => choice.layout(&sizes),
            None => (Canvas::default(), vec![(0, 0); sizes.len()]),
        };
        let mut project = Project::with_canvas(canvas);
        for ((path, &(w, h)), &(x, y)) in paths.iter().zip(&sizes).zip(&positions) {
            let source = project.add_source(path.clone());
            let id = project.state().next_item_id();
            project
                .apply(Step::AddItem {
                    id,
                    source,
                    x,
                    y,
                    source_width: w,
                    source_height: h,
                })
                .map_err(|e| e.to_string())?;
        }
        project.mark_saved();
        self.replace_project(project, None);
        if let Some(path) = paths.last() {
            self.remember_folder(path);
        }
        Ok(())
    }

    /// Adds images on top of the z-order (PLACE-001). Dropped images are centered on the
    /// drop point (`at`, screen coordinates); otherwise they go to the right of the
    /// existing images on a fitted canvas, or centered on a fixed canvas.
    pub fn add_images(&mut self, paths: &[PathBuf], at: Option<Point>) -> Result<(), String> {
        if self.is_empty() && self.path.is_none() && !self.project.is_dirty() {
            return self.open_images(paths);
        }
        for path in paths {
            let path = std::path::absolute(path).map_err(|e| e.to_string())?;
            let (w, h) = self.image_size(&path)?;
            let state = self.project.state();
            let canvas = state.canvas_rect();
            let (x, y) = match (at, state.canvas.size) {
                (Some(screen), _) => {
                    let p = self.view.to_canvas(screen);
                    (
                        (p.x - f64::from(w) / 2.0).round() as i32,
                        (p.y - f64::from(h) / 2.0).round() as i32,
                    )
                }
                (None, CanvasSize::Fit) => (canvas.right() as i32, canvas.y as i32),
                (None, CanvasSize::Fixed { width, height }) => {
                    (center(width, w), center(height, h))
                }
            };
            self.remember_folder(&path);
            let source = self.project.add_source(path);
            let id = self.project.state().next_item_id();
            if !self.record(Step::AddItem {
                id,
                source,
                x,
                y,
                source_width: w,
                source_height: h,
            }) {
                return Err(self.message.take().unwrap_or_default());
            }
            self.selection = Selection::Item(id);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_size() {
        assert_eq!(pixels_for_print_size(4.0, PrintUnit::Inches, 300.0), 1200);
        assert_eq!(
            pixels_for_print_size(2.54, PrintUnit::Centimeters, 300.0),
            300
        );
    }

    #[test]
    fn side_by_side_layout() {
        let (canvas, positions) = CanvasChoice::SideBySide.layout(&[(100, 50), (40, 80)]);
        assert_eq!(
            canvas.size,
            CanvasSize::Fixed {
                width: 140,
                height: 80
            }
        );
        assert_eq!(positions, vec![(0, 15), (100, 0)]);
    }
}
