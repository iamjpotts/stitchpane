//! The project: the source images and the steps applied to them (see
//! `docs/spec/formats/project-file.md`).

use std::path::PathBuf;

/// A source image referenced by a project.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    /// Path relative to the directory containing the project file.
    pub path: PathBuf,
}

/// An in-memory project.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Project {
    sources: Vec<Source>,
}

impl Project {
    /// Creates an empty project (spec: PROJ-001).
    pub fn new() -> Self {
        Self::default()
    }

    pub fn sources(&self) -> &[Source] {
        &self.sources
    }
}
