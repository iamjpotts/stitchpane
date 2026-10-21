//! Step definitions shared by all features.

mod project;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use imaginer_core::model::ItemId;
use imaginer_core::project::Project;

/// State of one scenario: a project plus a scratch directory for its files.
#[derive(Debug, cucumber::World)]
#[world(init = Self::new)]
pub struct World {
    pub dir: PathBuf,
    pub project: Project,
    /// Most recently added item ("the image").
    pub item: Option<ItemId>,
    pub error: Option<String>,
    pub snapshots: Vec<(PathBuf, Vec<u8>)>,
}

impl World {
    fn new() -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "imaginer-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).expect("create scratch directory");
        Self {
            dir,
            project: Project::new(),
            item: None,
            error: None,
            snapshots: Vec::new(),
        }
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.dir.join(relative)
    }

    pub fn item(&self) -> ItemId {
        self.item.expect("no image; add one first")
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
