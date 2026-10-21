//! Reading and writing `.imaginer` project files (`docs/spec/formats/project-file.md`).

use std::fmt;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::{Canvas, SourceId, Step, StepError};
use crate::project::{Project, Source};

/// Project file format version written by this build (PROJ-003).
pub const FORMAT_VERSION: u32 = 1;

/// File extension of project files (PROJ-002).
pub const EXTENSION: &str = "imaginer";

#[derive(Debug)]
pub enum LoadError {
    Io(std::io::Error),
    /// Malformed YAML, or an unrecognized field or step type (PROJ-007).
    Parse(String),
    UnsupportedVersion {
        found: u32,
        supported: u32,
    },
    InvalidStep {
        index: usize,
        error: StepError,
    },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Io(e) => write!(f, "could not read the project file: {e}"),
            LoadError::Parse(e) => write!(f, "the project file is not valid: {e}"),
            LoadError::UnsupportedVersion { found, supported } => write!(
                f,
                "the project file is version {found}, but this version of Imaginer supports \
                 up to version {supported}"
            ),
            LoadError::InvalidStep { index, error } => {
                write!(
                    f,
                    "step {} of the project file is invalid: {error}",
                    index + 1
                )
            }
        }
    }
}

impl std::error::Error for LoadError {}

#[derive(Debug)]
pub enum SaveError {
    Io(std::io::Error),
    Serialize(String),
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SaveError::Io(e) => write!(f, "could not write the project file: {e}"),
            SaveError::Serialize(e) => write!(f, "could not encode the project: {e}"),
        }
    }
}

impl std::error::Error for SaveError {}

#[derive(Deserialize)]
struct VersionOnly {
    version: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceEntry {
    id: SourceId,
    path: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectFile {
    version: u32,
    canvas: Canvas,
    sources: Vec<SourceEntry>,
    #[serde(with = "yaml_serde::with::singleton_map_recursive")]
    steps: Vec<Step>,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        with = "yaml_serde::with::singleton_map_recursive"
    )]
    redo: Vec<Step>,
}

/// Parses a project file's contents. Relative source paths are resolved against `dir`,
/// the directory containing the project file (PROJ-004).
pub fn from_str(text: &str, dir: &Path) -> Result<Project, LoadError> {
    let version: VersionOnly =
        yaml_serde::from_str(text).map_err(|e| LoadError::Parse(e.to_string()))?;
    if version.version > FORMAT_VERSION || version.version == 0 {
        return Err(LoadError::UnsupportedVersion {
            found: version.version,
            supported: FORMAT_VERSION,
        });
    }
    let file: ProjectFile =
        yaml_serde::from_str(text).map_err(|e| LoadError::Parse(e.to_string()))?;
    let resolve = |p: &str| normalize(&dir.join(p));
    let sources = file
        .sources
        .iter()
        .map(|s| Source {
            id: s.id,
            path: resolve(&s.path),
        })
        .collect();
    let absolute = |steps: Vec<Step>| -> Vec<Step> {
        steps
            .into_iter()
            .map(|mut step| {
                if let Step::RelinkSource { path, .. } = &mut step {
                    *path = resolve(&path.to_string_lossy());
                }
                step
            })
            .collect()
    };
    Project::from_parts(
        file.canvas,
        sources,
        absolute(file.steps),
        absolute(file.redo),
    )
    .map_err(|(index, error)| LoadError::InvalidStep { index, error })
}

/// Serializes a project for a project file located in `dir` (PROJ-002, PROJ-004, PROJ-008).
pub fn to_string(project: &Project, dir: &Path) -> Result<String, SaveError> {
    let relative = |p: &Path| relative_path(dir, p);
    let used: Vec<SourceId> = project
        .steps()
        .iter()
        .chain(project.redo_steps())
        .filter_map(|step| match step {
            Step::AddItem { source, .. } | Step::RelinkSource { source, .. } => Some(*source),
            _ => None,
        })
        .collect();
    let relink = |step: &Step| -> Step {
        let mut step = step.clone();
        if let Step::RelinkSource { path, .. } = &mut step {
            *path = PathBuf::from(relative(path));
        }
        step
    };
    let file = ProjectFile {
        version: FORMAT_VERSION,
        canvas: *project.canvas(),
        sources: project
            .sources()
            .iter()
            .filter(|s| used.contains(&s.id))
            .map(|s| SourceEntry {
                id: s.id,
                path: relative(&s.path),
            })
            .collect(),
        steps: project.steps().iter().map(relink).collect(),
        redo: project.redo_steps().map(relink).collect(),
    };
    yaml_serde::to_string(&file).map_err(|e| SaveError::Serialize(e.to_string()))
}

/// Reads a project file.
pub fn load(path: &Path) -> Result<Project, LoadError> {
    let text = std::fs::read_to_string(path).map_err(LoadError::Io)?;
    from_str(&text, &project_dir(path))
}

/// Writes a project file and marks the project saved.
pub fn save(project: &mut Project, path: &Path) -> Result<(), SaveError> {
    let text = to_string(project, &project_dir(path))?;
    std::fs::write(path, text).map_err(SaveError::Io)?;
    project.mark_saved();
    Ok(())
}

fn project_dir(path: &Path) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    normalize(absolute.parent().unwrap_or(Path::new("/")))
}

/// Resolves `.` and `..` components without touching the file system.
pub fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// `target` relative to the directory `base`, with `/` separators, using `..` where
/// needed (PROJ-004). Both paths should be absolute.
pub fn relative_path(base: &Path, target: &Path) -> String {
    let base = normalize(base);
    let target = normalize(target);
    let base: Vec<_> = base.components().collect();
    let target: Vec<_> = target.components().collect();
    let common = base.iter().zip(&target).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<String> = Vec::new();
    for _ in common..base.len() {
        parts.push("..".to_owned());
    }
    for component in &target[common..] {
        parts.push(component.as_os_str().to_string_lossy().into_owned());
    }
    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_paths() {
        let base = Path::new("/home/me/projects");
        assert_eq!(
            relative_path(base, Path::new("/home/me/projects/a/b.png")),
            "a/b.png"
        );
        assert_eq!(
            relative_path(base, Path::new("/home/me/photos/c.jpg")),
            "../photos/c.jpg"
        );
        assert_eq!(
            normalize(&base.join("../photos/./c.jpg")),
            PathBuf::from("/home/me/photos/c.jpg")
        );
    }
}
