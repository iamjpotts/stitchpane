//! The editor: everything both UIs share. It owns the project and its file, the view,
//! selection, tools and in-progress gestures, and turns user input into steps. UIs forward
//! pointer and key events, show the frame it renders, and provide dialogs for the
//! commands it cannot complete alone.

mod compose;
mod crop;
mod keys;
mod overlay;
mod pointer;
mod smear;
pub mod view;

use std::borrow::Cow;
use std::path::{Path, PathBuf};

pub use compose::{CanvasChoice, Preset, PrintUnit, pixels_for_print_size, proportional};
pub use crop::{AspectChoice, CropTool};
pub use keys::{Key, KeyOutcome, Modifiers};
pub use pointer::{Button, Cursor};
pub use smear::SmearTool;
use view::View;

use crate::codec::{self, EncodeOptions, Format};
use crate::metadata::{ExifEntry, ExifGroup};
use crate::model::{
    Curve, Feather, Filter, ItemId, Orientation, ProjectState, SourceId, Step, TransitionId,
    TransitionKind,
};
use crate::project::Project;
use crate::project_file;
use crate::render::{Renderer, SourceProblem};

/// Sides of an item, for feathered edges.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Selection {
    None,
    Item(ItemId),
    Transition(TransitionId),
    /// One feathered edge of an item (UI-006 adjusts its width).
    FeatherEdge(ItemId, Edge),
}

impl Selection {
    pub fn item(self) -> Option<ItemId> {
        match self {
            Selection::Item(id) | Selection::FeatherEdge(id, _) => Some(id),
            _ => None,
        }
    }
}

/// Stacking order changes (PLACE-005).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZOrder {
    Front,
    Forward,
    Backward,
    Back,
}

impl ZOrder {
    pub fn label(self) -> &'static str {
        match self {
            ZOrder::Front => "Bring to front",
            ZOrder::Forward => "Bring forward",
            ZOrder::Backward => "Send backward",
            ZOrder::Back => "Send to back",
        }
    }
}

/// Things the user can ask for from menus, shortcuts and buttons. Commands for which
/// [`Command::needs_ui`] is true need a dialog from the UI first; the rest are done by
/// [`Editor::execute`].
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    // Need a dialog.
    Open,
    AddImages,
    Save,
    SaveAs,
    Export,
    CanvasSize,
    SetDpi,
    Resize(ItemId),
    PrintSize(ItemId),
    RotateBy(ItemId),
    Feather(ItemId),
    TransitionProperties(TransitionId),
    ExifViewer(Option<ItemId>),
    LocateSource(SourceId),
    // Done by the editor.
    Undo,
    Redo,
    ZoomIn,
    ZoomOut,
    ZoomFit,
    Zoom100,
    Crop(ItemId),
    ApplyCrop,
    CancelCrop,
    Smear(ItemId),
    EndSmear,
    RemoveItem(ItemId),
    ToggleSnapping,
    ZOrder(ItemId, ZOrder),
    QuarterTurns(ItemId, i32),
    AddSeam([ItemId; 2], Orientation),
    AddCrossfade([ItemId; 2]),
    SwitchTransition(TransitionId),
    RemoveTransition(TransitionId),
}

impl Command {
    pub fn needs_ui(&self) -> bool {
        matches!(
            self,
            Command::Open
                | Command::AddImages
                | Command::Save
                | Command::SaveAs
                | Command::Export
                | Command::CanvasSize
                | Command::SetDpi
                | Command::Resize(_)
                | Command::PrintSize(_)
                | Command::RotateBy(_)
                | Command::Feather(_)
                | Command::TransitionProperties(_)
                | Command::ExifViewer(_)
                | Command::LocateSource(_)
        )
    }
}

/// A context menu row; `command: None` is a separator.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuEntry {
    pub label: String,
    pub command: Option<Command>,
}

impl MenuEntry {
    fn new(label: impl Into<String>, command: Command) -> Self {
        Self {
            label: label.into(),
            command: Some(command),
        }
    }

    fn separator() -> Self {
        Self {
            label: String::new(),
            command: None,
        }
    }
}

/// Export dialog contents (EXPORT-001, CONV-003, META-005).
#[derive(Clone, Debug, PartialEq)]
pub struct ExportSettings {
    pub path: PathBuf,
    pub format: Format,
    pub quality: u8,
    /// Whose EXIF to include, if any.
    pub metadata: Option<ItemId>,
    pub groups: Vec<ExifGroup>,
}

/// An in-progress keyboard adjustment (UI-006): one step per run of key presses.
#[derive(Clone, Copy, Debug, PartialEq)]
enum AdjustTarget {
    Move(ItemId),
    Resize(ItemId),
    Rotate(ItemId),
    Seam(TransitionId),
    Feather(ItemId),
}

pub struct Editor {
    project: Project,
    path: Option<PathBuf>,
    renderer: Renderer,
    view: View,
    selection: Selection,
    crop: Option<CropTool>,
    /// The item the smear tool is active on (SMEAR-001).
    smear: Option<ItemId>,
    smear_settings: smear::SmearSettings,
    /// Last pointer position over the canvas, for the brush outline.
    hover: Option<crate::geometry::Point>,
    gesture: Option<pointer::Gesture>,
    pending: Option<(AdjustTarget, Step)>,
    snapping: bool,
    frame_cache: Option<overlay::FrameCache>,
    /// Background and checkerboard, keyed by viewport size and canvas position.
    background: Option<(overlay::BackgroundKey, image::RgbaImage)>,
    message: Option<String>,
    /// Folder of the file most recently picked, opened, saved or exported (UI-007).
    last_folder: Option<PathBuf>,
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

impl Editor {
    pub fn new() -> Self {
        Self::with_project(Project::new(), None)
    }

    fn with_project(project: Project, path: Option<PathBuf>) -> Self {
        let mut editor = Self {
            project,
            path,
            renderer: Renderer::new(),
            view: View::default(),
            selection: Selection::None,
            crop: None,
            smear: None,
            smear_settings: smear::SmearSettings::default(),
            hover: None,
            gesture: None,
            pending: None,
            snapping: true,
            frame_cache: None,
            background: None,
            message: None,
            last_folder: None,
        };
        editor.fit_view();
        editor
    }

    fn replace_project(&mut self, project: Project, path: Option<PathBuf>) {
        let viewport = self.view.viewport;
        let snapping = self.snapping;
        let last_folder = self.last_folder.take();
        let smear_settings = self.smear_settings;
        let background = self.renderer.background();
        *self = Self::with_project(project, path);
        self.snapping = snapping;
        self.last_folder = last_folder;
        self.smear_settings = smear_settings;
        self.renderer.set_background(background);
        self.set_viewport(viewport.0, viewport.1);
    }

    /// Makes resampling for the screen happen in a background thread, so drawing never
    /// waits for it (UI-009). The UI then calls [`Editor::poll_background`] while
    /// [`Editor::background_busy`] is true and redraws when it returns true.
    pub fn set_background_rendering(&mut self, enabled: bool) {
        self.renderer.set_background(enabled);
    }

    /// Whether background resamples are still being made (UI-009).
    pub fn background_busy(&self) -> bool {
        self.renderer.busy()
    }

    /// Takes finished background resamples. Returns true if the frame needs redrawing.
    pub fn poll_background(&mut self) -> bool {
        let arrived = self.renderer.poll();
        if arrived {
            self.frame_cache = None;
        }
        arrived
    }

    /// The folder file dialogs should start in (UI-007), if any file was picked yet.
    pub fn dialog_folder(&self) -> Option<&Path> {
        self.last_folder.as_deref()
    }

    /// Notes the file the user picked or saved, for [`Editor::dialog_folder`] (UI-007).
    pub fn remember_folder(&mut self, file: &Path) {
        let Ok(file) = std::path::absolute(file) else {
            return;
        };
        if let Some(folder) = file.parent() {
            self.last_folder = Some(folder.to_path_buf());
        }
    }

    pub fn project(&self) -> &Project {
        &self.project
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn view(&self) -> &View {
        &self.view
    }

    pub fn selection(&self) -> Selection {
        self.selection
    }

    pub fn select(&mut self, selection: Selection) {
        if selection != self.selection {
            self.commit_pending();
            self.selection = selection;
        }
    }

    pub fn crop_tool(&self) -> Option<&CropTool> {
        self.crop.as_ref()
    }

    pub fn snapping(&self) -> bool {
        self.snapping
    }

    pub fn is_empty(&self) -> bool {
        self.project.state().items.is_empty()
    }

    /// The last error, for the UI to show; cleared when read.
    pub fn take_message(&mut self) -> Option<String> {
        self.message.take()
    }

    /// Unsaved changes (UI-002), including an uncommitted keyboard adjustment.
    pub fn is_dirty(&self) -> bool {
        self.project.is_dirty() || self.pending.is_some()
    }

    /// Window title.
    pub fn title(&self) -> String {
        let name = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".to_owned());
        let dirty = if self.is_dirty() { "*" } else { "" };
        format!("{dirty}{name} — {}", crate::APP_NAME)
    }

    /// The state shown on screen: the project with any in-progress gesture or keyboard
    /// adjustment applied. While cropping, the item shows its whole source on top of the
    /// other items (CROP-005).
    pub fn display_state(&self) -> Cow<'_, ProjectState> {
        let preview = self
            .gesture
            .as_ref()
            .and_then(pointer::Gesture::preview)
            .or(self.pending.as_ref().map(|(_, step)| step));
        let mut state = match preview {
            Some(step) => {
                let mut state = self.project.state().clone();
                if state.apply(step).is_ok() {
                    Cow::Owned(state)
                } else {
                    Cow::Borrowed(self.project.state())
                }
            }
            None => Cow::Borrowed(self.project.state()),
        };
        if let Some(tool) = &self.crop
            && let Some(index) = state.items.iter().position(|i| i.id == tool.item)
        {
            let items = &mut state.to_mut().items;
            let item = items.remove(index);
            items.push(crop::with_crop(&item, tool.limits));
        }
        state
    }

    /// Records a step from a dialog or other UI action. Returns false (and sets the
    /// message) if the step is invalid.
    pub fn apply(&mut self, step: Step) -> bool {
        self.record(step)
    }

    /// Sets zoom and scroll position directly.
    pub fn set_view(&mut self, zoom: f64, pan: crate::geometry::Point) {
        self.view.zoom = zoom.clamp(view::MIN_ZOOM, view::MAX_ZOOM);
        self.view.pan = pan;
        self.view.fit = false;
    }

    /// Renders the whole canvas at full size, as export does.
    pub fn render_canvas(&mut self) -> image::RgbaImage {
        let state = self.display_state().into_owned();
        self.renderer.render(&state)
    }

    /// Records a step, reporting a rejected step as the message.
    fn record(&mut self, step: Step) -> bool {
        self.commit_pending();
        match self.project.apply(step) {
            Ok(()) => {
                self.after_change();
                true
            }
            Err(e) => {
                self.message = Some(e.to_string());
                false
            }
        }
    }

    /// Ends a keyboard adjustment run, recording its step (UNDO-004, UI-006).
    pub fn commit_pending(&mut self) {
        if let Some((_, step)) = self.pending.take()
            && let Err(e) = self.project.apply(step)
        {
            self.message = Some(e.to_string());
        }
        self.after_change();
    }

    fn after_change(&mut self) {
        let state = self.project.state();
        // Drop a selection whose target no longer exists (for example after undo).
        let valid = match self.selection {
            Selection::None => true,
            Selection::Item(id) | Selection::FeatherEdge(id, _) => state.item(id).is_some(),
            Selection::Transition(id) => state.transition(id).is_some(),
        };
        if !valid {
            self.selection = Selection::None;
        }
        if let Some(crop) = &self.crop
            && state.item(crop.item).is_none()
        {
            self.crop = None;
        }
        if self.smear.is_some_and(|item| state.item(item).is_none()) {
            self.smear = None;
        }
        if self.view.fit {
            self.fit_view();
        }
    }

    // ----- view -----

    pub fn set_viewport(&mut self, width: f64, height: f64) {
        self.view.viewport = (width.max(1.0), height.max(1.0));
        if self.view.fit {
            self.fit_view();
        }
    }

    fn fit_view(&mut self) {
        let canvas = self.project.state().canvas_rect();
        self.view.fit_to(canvas);
    }

    // ----- commands -----

    /// Performs a command that needs no dialog. Returns false if it could not be done.
    pub fn execute(&mut self, command: Command) -> bool {
        debug_assert!(!command.needs_ui(), "{command:?} needs a dialog");
        self.commit_pending();
        match command {
            Command::Undo => self.undo(),
            Command::Redo => self.redo(),
            Command::ZoomIn => {
                let c = self.view.viewport_center();
                self.view.zoom_by(view::ZOOM_STEP, c);
                true
            }
            Command::ZoomOut => {
                let c = self.view.viewport_center();
                self.view.zoom_by(1.0 / view::ZOOM_STEP, c);
                true
            }
            Command::ZoomFit => {
                self.fit_view();
                true
            }
            Command::Zoom100 => {
                let c = self.view.viewport_center();
                self.view.set_zoom(1.0, c);
                true
            }
            Command::Crop(item) => {
                self.smear = None;
                self.start_crop(item)
            }
            Command::Smear(item) => self.start_smear(item),
            Command::EndSmear => {
                self.smear = None;
                true
            }
            Command::RemoveItem(item) => self.record(Step::RemoveItem { item }),
            Command::ApplyCrop => self.apply_crop(),
            Command::CancelCrop => {
                self.crop = None;
                true
            }
            Command::ToggleSnapping => {
                self.snapping = !self.snapping;
                true
            }
            Command::ZOrder(item, order) => self.z_order(item, order),
            Command::QuarterTurns(item, turns) => self.quarter_turns(item, turns),
            Command::AddSeam(items, orientation) => self.add_seam(items, orientation),
            Command::AddCrossfade(items) => self.add_transition(
                items,
                TransitionKind::Crossfade {
                    ratio: DEFAULT_CROSSFADE,
                },
            ),
            Command::SwitchTransition(id) => self.switch_transition(id),
            Command::RemoveTransition(id) => self.record(Step::RemoveTransition { id }),
            other => {
                self.message = Some(format!("{other:?} needs a dialog"));
                false
            }
        }
    }

    pub fn can_undo(&self) -> bool {
        self.pending.is_some() || self.project.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.pending.is_none() && self.project.can_redo()
    }

    /// "Undo <step>" for the menu and tooltip (UNDO-003).
    pub fn undo_label(&self) -> Option<String> {
        match &self.pending {
            Some((_, step)) => Some(format!("Undo {}", step.label())),
            None => self.project.undo_label(),
        }
    }

    pub fn redo_label(&self) -> Option<String> {
        if self.pending.is_some() {
            return None;
        }
        self.project.redo_label()
    }

    pub fn undo(&mut self) -> bool {
        self.commit_pending();
        self.crop = None;
        let done = self.project.undo();
        self.after_change();
        done
    }

    pub fn redo(&mut self) -> bool {
        self.commit_pending();
        self.crop = None;
        let done = self.project.redo();
        self.after_change();
        done
    }

    fn z_order(&mut self, item: ItemId, order: ZOrder) -> bool {
        let state = self.project.state();
        let Some(from) = state.item_index(item) else {
            return false;
        };
        let last = state.items.len() - 1;
        let index = match order {
            ZOrder::Front => last,
            ZOrder::Forward => (from + 1).min(last),
            ZOrder::Backward => from.saturating_sub(1),
            ZOrder::Back => 0,
        };
        if index == from {
            return true;
        }
        self.record(Step::SetZOrder { item, index })
    }

    /// Rotates by quarter turns without resampling (PLACE-007).
    fn quarter_turns(&mut self, item: ItemId, turns: i32) -> bool {
        let Some(current) = self.project.state().item(item) else {
            return false;
        };
        let degrees = (current.rotation + 90.0 * f64::from(turns)).rem_euclid(360.0);
        self.record(Step::RotateItem { item, degrees })
    }

    /// Rotates to an angle in degrees, to 0.1° (PLACE-008).
    pub fn rotate_to(&mut self, item: ItemId, degrees: f64) -> bool {
        let degrees = round_tenth(degrees).rem_euclid(360.0);
        self.record(Step::RotateItem { item, degrees })
    }

    fn add_seam(&mut self, items: [ItemId; 2], orientation: Orientation) -> bool {
        let Some(overlap) = self.project.state().overlap(items[0], items[1]) else {
            self.message = Some("the images do not overlap".to_owned());
            return false;
        };
        // BLEND-001: centered seam, band half the overlap's extent.
        let extent = match orientation {
            Orientation::Horizontal => overlap.width,
            Orientation::Vertical => overlap.height,
        };
        self.add_transition(
            items,
            TransitionKind::GradientSeam {
                orientation,
                offset: 0.0,
                width: (extent / 2.0).round(),
                curve: Curve::Linear,
            },
        )
    }

    fn add_transition(&mut self, items: [ItemId; 2], kind: TransitionKind) -> bool {
        if self.project.state().overlap(items[0], items[1]).is_none() {
            self.message = Some("the images do not overlap".to_owned());
            return false;
        }
        let id = self.project.state().next_transition_id();
        let added = self.record(Step::AddTransition { id, items, kind });
        if added {
            self.selection = Selection::Transition(id);
        }
        added
    }

    /// Switches a transition between gradient seam and crossfade (BLEND-003).
    fn switch_transition(&mut self, id: TransitionId) -> bool {
        let state = self.project.state();
        let Some(t) = state.transition(id).copied() else {
            return false;
        };
        let kind = match t.kind {
            TransitionKind::GradientSeam { .. } => TransitionKind::Crossfade {
                ratio: DEFAULT_CROSSFADE,
            },
            TransitionKind::Crossfade { .. } => {
                let overlap = state.overlap(t.items[0], t.items[1]);
                TransitionKind::GradientSeam {
                    orientation: Orientation::Horizontal,
                    offset: 0.0,
                    width: overlap.map(|o| (o.width / 2.0).round()).unwrap_or(10.0),
                    curve: Curve::Linear,
                }
            }
        };
        self.record(Step::EditTransition { id, kind })
    }

    /// Changes a transition's properties (BLEND-003, BLEND-008).
    pub fn edit_transition(&mut self, id: TransitionId, kind: TransitionKind) -> bool {
        if self.project.state().transition(id).map(|t| t.kind) == Some(kind) {
            return true;
        }
        self.record(Step::EditTransition { id, kind })
    }

    /// Sets or clears an item's feathered edges (BLEND-007).
    pub fn set_feather(&mut self, item: ItemId, feather: Option<Feather>) -> bool {
        let feather = feather.filter(Feather::any_edge);
        self.record(Step::SetFeather { item, feather })
    }

    /// Default feather for the dialog: 10% of the item's shorter side (BLEND-007).
    pub fn default_feather(&self, item: ItemId) -> Feather {
        let state = self.project.state();
        let current = state.item(item);
        current.and_then(|i| i.feather).unwrap_or_else(|| Feather {
            left: false,
            right: false,
            top: false,
            bottom: false,
            width: current
                .map(|i| (f64::from(i.width.min(i.height)) * 0.1).round().max(1.0))
                .unwrap_or(10.0),
            curve: Curve::Linear,
        })
    }

    /// Resamples an item to new pixel dimensions (RES-001, RES-002).
    pub fn resize_item(&mut self, item: ItemId, width: u32, height: u32, filter: Filter) -> bool {
        let Some(current) = self.project.state().item(item).copied() else {
            return false;
        };
        self.record(Step::ResizeItem {
            item,
            x: current.x,
            y: current.y,
            width,
            height,
            filter,
        })
    }

    /// Sets the canvas DPI without resampling (RES-003).
    pub fn set_dpi(&mut self, dpi: Option<f64>) -> bool {
        if self.project.state().canvas.dpi == dpi {
            return true;
        }
        self.record(Step::SetDpi { dpi })
    }

    /// Resamples to a print size at a DPI (RES-004).
    pub fn set_print_size(
        &mut self,
        item: ItemId,
        width: f64,
        height: f64,
        unit: PrintUnit,
        dpi: f64,
        filter: Filter,
    ) -> bool {
        let width = pixels_for_print_size(width, unit, dpi);
        let height = pixels_for_print_size(height, unit, dpi);
        self.record(Step::SetPrintSize {
            item,
            width,
            height,
            filter,
            dpi,
        })
    }

    /// Sets the canvas size and DPI from the "Canvas size…" dialog (CANVAS-002).
    pub fn set_canvas_size(&mut self, width: u32, height: u32, dpi: Option<f64>) -> bool {
        // Keep the visible content where it is: the canvas origin becomes the current one.
        let current = self.project.state().canvas_rect();
        let (offset_x, offset_y) = (-current.x as i32, -current.y as i32);
        self.record(Step::ResizeCanvas {
            width,
            height,
            offset_x,
            offset_y,
            dpi,
        })
    }

    pub fn strip_metadata(&mut self, item: ItemId) -> bool {
        self.record(Step::StripMetadata { item })
    }

    // ----- files -----

    /// Opens a project file or an image (UI-001). An image starts a single-image project.
    pub fn open(&mut self, path: &Path) -> Result<(), String> {
        let path = std::path::absolute(path).map_err(|e| e.to_string())?;
        let is_project = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case(project_file::EXTENSION));
        if is_project {
            let project = project_file::load(&path).map_err(|e| e.to_string())?;
            self.replace_project(project, Some(path.clone()));
            self.remember_folder(&path);
            Ok(())
        } else {
            self.open_images(&[path])
        }
    }

    /// Opens one image as a single-image project, or several as a side-by-side
    /// composition. Use [`Editor::new_composition`] to choose the canvas (CANVAS-001).
    pub fn open_images(&mut self, paths: &[PathBuf]) -> Result<(), String> {
        let choice = if paths.len() == 1 {
            None
        } else {
            Some(CanvasChoice::SideBySide)
        };
        self.new_composition(paths, choice)
    }

    /// Unsaved-changes check before replacing the project, for the UI (UI-002).
    pub fn needs_save_prompt(&self) -> bool {
        self.is_dirty() && !self.is_empty()
    }

    pub fn save(&mut self) -> Result<(), String> {
        let path = self.path.clone().ok_or("the project has no file yet")?;
        self.save_as(&path)
    }

    /// Saves to a path, adding the `.stitchpane` extension if missing (PROJ-002).
    pub fn save_as(&mut self, path: &Path) -> Result<(), String> {
        self.commit_pending();
        let mut path = std::path::absolute(path).map_err(|e| e.to_string())?;
        if !path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case(project_file::EXTENSION))
        {
            let mut name = path.file_name().unwrap_or_default().to_os_string();
            name.push(".");
            name.push(project_file::EXTENSION);
            path.set_file_name(name);
        }
        project_file::save(&mut self.project, &path).map_err(|e| e.to_string())?;
        self.remember_folder(&path);
        self.path = Some(path);
        Ok(())
    }

    /// Display name of an item: its source file name.
    pub fn item_name(&self, item: ItemId) -> String {
        let state = self.project.state();
        state
            .item(item)
            .and_then(|i| state.sources.get(&i.source))
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| item.to_string())
    }

    /// Items whose source has EXIF metadata that has not been stripped.
    pub fn items_with_exif(&mut self) -> Vec<ItemId> {
        let state = self.project.state().clone();
        state
            .items
            .iter()
            .filter(|item| !item.metadata_stripped)
            .filter(|item| {
                state
                    .sources
                    .get(&item.source)
                    .and_then(|p| self.renderer.source(p).ok())
                    .is_some_and(|s| s.exif.is_some())
            })
            .map(|item| item.id)
            .collect()
    }

    /// EXIF rows of an item for the viewer (META-003).
    pub fn exif_entries(&mut self, item: ItemId) -> Vec<ExifEntry> {
        self.exif_of(item)
            .map(|e| e.entries().to_vec())
            .unwrap_or_default()
    }

    /// EXIF groups present in an item's metadata, for the export checkboxes.
    pub fn exif_groups(&mut self, item: ItemId) -> Vec<ExifGroup> {
        self.exif_of(item).map(|e| e.groups()).unwrap_or_default()
    }

    fn exif_of(&mut self, item: ItemId) -> Option<crate::metadata::Exif> {
        let state = self.project.state();
        let item = state.item(item)?;
        if item.metadata_stripped {
            return None;
        }
        let path = state.sources.get(&item.source)?.clone();
        self.renderer.source(&path).ok()?.exif.clone()
    }

    /// Status bar notice about EXIF metadata (META-001).
    pub fn exif_notice(&mut self) -> Option<String> {
        if let Some(item) = self.selection.item() {
            return (!self.exif_entries(item).is_empty())
                .then(|| "Contains EXIF metadata".to_owned());
        }
        match self.items_with_exif().len() {
            0 => None,
            1 if self.project.state().items.len() == 1 => Some("Contains EXIF metadata".to_owned()),
            1 => Some("1 image contains EXIF metadata".to_owned()),
            n => Some(format!("{n} images contain EXIF metadata")),
        }
    }

    /// Item whose EXIF the viewer opens with when the notice is clicked (META-002).
    pub fn exif_viewer_item(&mut self) -> Option<ItemId> {
        let with_exif = self.items_with_exif();
        self.selection
            .item()
            .filter(|i| with_exif.contains(i))
            .or_else(|| with_exif.first().copied())
    }

    /// Sources that cannot be loaded, with their file names (PROJ-005).
    pub fn missing_sources(&mut self) -> Vec<(SourceId, String, SourceProblem)> {
        let state = self.project.state().clone();
        self.renderer
            .problems(&state)
            .into_iter()
            .map(|(id, problem)| {
                let name = state
                    .sources
                    .get(&id)
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                (id, name, problem)
            })
            .collect()
    }

    /// Points a missing source at a new file (PROJ-005).
    pub fn relink(&mut self, source: SourceId, path: &Path) -> bool {
        let Ok(path) = std::path::absolute(path) else {
            return false;
        };
        self.remember_folder(&path);
        self.renderer.forget(&path);
        if let Some(old) = self.project.state().sources.get(&source).cloned() {
            self.renderer.forget(&old);
        }
        self.record(Step::RelinkSource { source, path })
    }

    /// Default export settings (META-005: one item's EXIF when only one item has any).
    pub fn export_defaults(&mut self, path: &Path) -> ExportSettings {
        let with_exif = self.items_with_exif();
        let metadata =
            (self.project.state().items.len() == 1 && with_exif.len() == 1).then(|| with_exif[0]);
        ExportSettings {
            path: path.to_path_buf(),
            format: Format::from_path(path).unwrap_or(Format::Png),
            quality: codec::DEFAULT_QUALITY,
            metadata,
            groups: ExifGroup::ALL
                .into_iter()
                .filter(|g| g.exported_by_default())
                .collect(),
        }
    }

    /// Whether exporting to `path` would overwrite a source (EXPORT-003).
    pub fn export_overwrites_source(&self, path: &Path) -> bool {
        let Ok(path) = std::path::absolute(path) else {
            return false;
        };
        let path = project_file::normalize(&path);
        self.project
            .state()
            .sources
            .values()
            .any(|s| project_file::normalize(s) == path)
    }

    /// Renders the project and writes it to a file (EXPORT-001 to EXPORT-004, META-005).
    pub fn export(&mut self, settings: &ExportSettings) -> Result<(), String> {
        self.commit_pending();
        let state = self.project.state().clone();
        if state.items.is_empty() {
            return Err("there is nothing to export".to_owned());
        }
        if let Some((_, name, _)) = self.missing_sources().first() {
            return Err(format!("the source image {name} is missing"));
        }
        let pixels = self.renderer.render(&state);
        let exif = settings.metadata.and_then(|item| {
            self.exif_of(item)?.export(
                &settings.groups,
                pixels.width(),
                pixels.height(),
                state.canvas.dpi,
            )
        });
        let options = EncodeOptions {
            format: settings.format,
            quality: settings.quality,
            dpi: state.canvas.dpi,
            exif: exif.filter(|_| settings.format.supports_exif()),
        };
        codec::write(&pixels, &settings.path, &options).map_err(|e| e.to_string())?;
        self.remember_folder(&settings.path);
        Ok(())
    }

    /// Status bar text: canvas size, DPI and zoom.
    pub fn status_text(&self) -> String {
        format!("{} · {}", self.canvas_text(), self.zoom_text())
    }

    /// Status bar text without the zoom: canvas size and DPI.
    pub fn canvas_text(&self) -> String {
        let state = self.display_state();
        let (w, h) = state.canvas_size();
        let mut text = format!("{w} × {h} px");
        if let Some(dpi) = state.canvas.dpi {
            text.push_str(&format!(" · {} DPI", format_number(dpi)));
        }
        text
    }

    /// The zoom level as the status bar shows it; clicking it opens the zoom menu (UI-010).
    pub fn zoom_text(&self) -> String {
        format!("zoom {}%", (self.view.zoom * 100.0).round())
    }

    /// The View menu's zoom choices, also offered from the status bar (UI-010).
    pub fn zoom_menu() -> Vec<MenuEntry> {
        vec![
            MenuEntry::new("Zoom in", Command::ZoomIn),
            MenuEntry::new("Zoom out", Command::ZoomOut),
            MenuEntry::new("Fit", Command::ZoomFit),
            MenuEntry::new("100%", Command::Zoom100),
        ]
    }
}

/// Default crossfade ratio (BLEND-008).
pub const DEFAULT_CROSSFADE: f64 = 0.5;

fn round_tenth(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

/// Formats a number without a trailing `.0`.
pub fn format_number(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.1}")
    }
}
