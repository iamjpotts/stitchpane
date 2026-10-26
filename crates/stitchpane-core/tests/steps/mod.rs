//! Step definitions shared by all features. Scenarios drive the [`Editor`], which is what
//! both UIs use, so UI behavior that lives in the editor is tested here.

mod checks;
mod editing;
mod files;
mod light;
mod project;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use stitchpane_core::editor::{Editor, ExportSettings, MenuEntry};
use stitchpane_core::geometry::Point;
use stitchpane_core::model::ItemId;

/// State of one scenario: an editor plus a scratch directory for its files.
#[derive(cucumber::World)]
#[world(init = Self::new)]
pub struct World {
    pub dir: PathBuf,
    pub editor: Editor,
    /// Most recently added item ("the image").
    pub item: Option<ItemId>,
    pub error: Option<String>,
    pub snapshots: Vec<(PathBuf, Vec<u8>)>,
    pub menu: Vec<MenuEntry>,
    pub export: Option<ExportSettings>,
    /// A remembered screen position, for "stays in place" checks.
    pub remembered: Option<Point>,
    /// A remembered rendering of the canvas, for "lighter than before" checks.
    pub remembered_canvas: Option<image::RgbaImage>,
    pub images: Vec<PathBuf>,
    /// A command the editor handed back to the UI (it needs a dialog).
    pub ui_request: Option<stitchpane_core::editor::Command>,
}

impl std::fmt::Debug for World {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("World").field("dir", &self.dir).finish()
    }
}

impl World {
    fn new() -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "stitchpane-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).expect("create scratch directory");
        let mut editor = Editor::new();
        editor.set_viewport(1000.0, 800.0);
        Self {
            dir,
            editor,
            item: None,
            error: None,
            snapshots: Vec::new(),
            menu: Vec::new(),
            export: None,
            remembered: None,
            remembered_canvas: None,
            images: Vec::new(),
            ui_request: None,
        }
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.dir.join(relative)
    }

    pub fn item(&self) -> ItemId {
        self.item
            .or_else(|| self.editor.selection().item())
            .or_else(|| self.editor.project().state().items.last().map(|i| i.id))
            .expect("no image")
    }

    /// Screen position of a canvas point.
    pub fn screen(&self, x: f64, y: f64) -> Point {
        self.editor.view().to_screen(Point::new(x, y))
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

pub fn ensure_parent(path: &Path) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create directory");
    }
}

pub fn color(name: &str) -> [u8; 4] {
    match name {
        "red" => [255, 0, 0, 255],
        "green" => [0, 255, 0, 255],
        "blue" => [0, 0, 255, 255],
        "white" => [255, 255, 255, 255],
        "gray" => [128, 128, 128, 255],
        "black" => [0, 0, 0, 255],
        "transparent" => [0, 0, 0, 0],
        other => panic!("unknown color {other}"),
    }
}

/// Whether two colors are equal within a tolerance per channel.
pub fn close(a: [u8; 4], b: [u8; 4], tolerance: u8) -> bool {
    a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= tolerance)
}
