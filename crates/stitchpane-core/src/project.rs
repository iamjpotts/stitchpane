//! The project: the source images and the steps applied to them, with undo and redo
//! history (see `docs/spec/formats/project-file.md` and `docs/spec/product/editing.md`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::model::{Canvas, ProjectState, SourceId, Step, StepError, next_id};

/// A source image referenced by a project.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub id: SourceId,
    /// Absolute path (relative paths exist only in the project file, PROJ-004).
    pub path: PathBuf,
}

/// An in-memory project.
#[derive(Clone, Debug, PartialEq)]
pub struct Project {
    sources: Vec<Source>,
    canvas: Canvas,
    steps: Vec<Step>,
    /// Next step to redo is last.
    redo: Vec<Step>,
    state: ProjectState,
    changes: u64,
    saved_changes: u64,
}

impl Default for Project {
    fn default() -> Self {
        Self::new()
    }
}

impl Project {
    /// Creates an empty project (spec: PROJ-001).
    pub fn new() -> Self {
        Self::with_canvas(Canvas::default())
    }

    pub fn with_canvas(canvas: Canvas) -> Self {
        Self {
            sources: Vec::new(),
            canvas,
            steps: Vec::new(),
            redo: Vec::new(),
            state: ProjectState::new(canvas, BTreeMap::new()),
            changes: 0,
            saved_changes: 0,
        }
    }

    /// Rebuilds a project from saved parts, replaying `steps` (PROJ-006, PROJ-008).
    /// `redo` is in file order: first entry is redone first.
    pub fn from_parts(
        canvas: Canvas,
        sources: Vec<Source>,
        steps: Vec<Step>,
        redo: Vec<Step>,
    ) -> Result<Self, (usize, StepError)> {
        let mut project = Self::with_canvas(canvas);
        project.sources = sources;
        project.state = project.initial_state();
        for (index, step) in steps.iter().enumerate() {
            project.state.apply(step).map_err(|e| (index, e))?;
        }
        project.steps = steps;
        project.redo = redo.into_iter().rev().collect();
        Ok(project)
    }

    pub fn sources(&self) -> &[Source] {
        &self.sources
    }

    pub fn source(&self, id: SourceId) -> Option<&Source> {
        self.sources.iter().find(|s| s.id == id)
    }

    /// Current path of a source, after any relinking.
    pub fn source_path(&self, id: SourceId) -> Option<&Path> {
        self.state.sources.get(&id).map(PathBuf::as_path)
    }

    pub fn canvas(&self) -> &Canvas {
        &self.canvas
    }

    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    /// Steps that can be redone, next first.
    pub fn redo_steps(&self) -> impl Iterator<Item = &Step> {
        self.redo.iter().rev()
    }

    pub fn state(&self) -> &ProjectState {
        &self.state
    }

    /// Registers a source image, reusing an existing entry for the same path.
    pub fn add_source(&mut self, path: impl Into<PathBuf>) -> SourceId {
        let path = path.into();
        // Match the current (possibly relinked) path, which is what gets rendered.
        if let Some((&id, _)) = self.state.sources.iter().find(|(_, p)| **p == path) {
            return id;
        }
        let id = SourceId(next_id(
            self.sources
                .iter()
                .map(|s| s.id.0)
                .chain(self.state.sources.keys().map(|k| k.0)),
        ));
        self.sources.push(Source {
            id,
            path: path.clone(),
        });
        self.state.sources.entry(id).or_insert(path);
        id
    }

    /// Applies and records a step. Discards the redo history (UNDO-002).
    pub fn apply(&mut self, step: Step) -> Result<(), StepError> {
        self.state.apply(&step)?;
        self.steps.push(step);
        self.redo.clear();
        self.changes = self.changes.wrapping_add(1);
        Ok(())
    }

    pub fn can_undo(&self) -> bool {
        !self.steps.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// "Undo <step>" (UNDO-003), or `None` when there is nothing to undo.
    pub fn undo_label(&self) -> Option<String> {
        self.steps.last().map(|s| format!("Undo {}", s.label()))
    }

    /// "Redo <step>" (UNDO-003), or `None` when there is nothing to redo.
    pub fn redo_label(&self) -> Option<String> {
        self.redo.last().map(|s| format!("Redo {}", s.label()))
    }

    /// Undoes the last step (UNDO-001).
    pub fn undo(&mut self) -> bool {
        let Some(step) = self.steps.pop() else {
            return false;
        };
        if let Err(step) = self.replay_without(step) {
            self.steps.push(step);
            return false;
        }
        self.changes = self.changes.wrapping_add(1);
        true
    }

    /// Redoes the last undone step (UNDO-002).
    pub fn redo(&mut self) -> bool {
        let Some(step) = self.redo.pop() else {
            return false;
        };
        if self.state.apply(&step).is_err() {
            self.redo.push(step);
            return false;
        }
        self.steps.push(step);
        self.changes = self.changes.wrapping_add(1);
        true
    }

    pub fn is_dirty(&self) -> bool {
        self.changes != self.saved_changes
    }

    pub fn mark_saved(&mut self) {
        self.saved_changes = self.changes;
    }

    fn initial_state(&self) -> ProjectState {
        ProjectState::new(
            self.canvas,
            self.sources
                .iter()
                .map(|s| (s.id, s.path.clone()))
                .collect(),
        )
    }

    /// Rebuilds the state from the recorded steps after `undone` was popped from them,
    /// and moves `undone` to the redo history. Recorded steps were valid when applied and
    /// replay is deterministic; if one fails anyway, the state is left unchanged and
    /// `undone` is handed back.
    fn replay_without(&mut self, undone: Step) -> Result<(), Step> {
        let mut state = self.initial_state();
        for step in &self.steps {
            if state.apply(step).is_err() {
                return Err(undone);
            }
        }
        self.state = state;
        self.redo.push(undone);
        Ok(())
    }
}
