//! Dialogs (drawn inside the window), file choosers and the status bar.

use std::path::PathBuf;

use iced::widget::{
    Space, button, checkbox, column, container, pick_list, radio, row, scrollable, text, text_input,
};
use iced::{Alignment, Element, Length, Task};
use stitchpane_core::codec::Format;
use stitchpane_core::editor::{
    CanvasChoice, Command, ExportSettings, Preset, PrintUnit, format_number, proportional,
};
use stitchpane_core::metadata::{ExifEntry, ExifGroup};
use stitchpane_core::model::{
    Curve, Feather, Filter, ItemId, Orientation, SourceId, TransitionId, TransitionKind,
};
use stitchpane_core::project_file;

use crate::{App, Message};

/// What a file chooser was opened for.
#[derive(Clone, Debug)]
pub enum Purpose {
    Open,
    AddImages,
    SaveAs(AfterDiscard),
    Export,
    Locate(SourceId),
}

/// What to do once unsaved changes are dealt with (UI-002).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AfterDiscard {
    Nothing,
    Open,
    Quit,
}

pub enum Modal {
    Error(String),
    ConfirmDiscard(AfterDiscard),
    ConfirmOverwrite(PathBuf),
    CanvasChoice {
        paths: Vec<PathBuf>,
        choice: usize,
        size: [String; 3],
        use_dpi: bool,
    },
    CanvasSize {
        size: [String; 3],
        use_dpi: bool,
    },
    SetDpi {
        dpi: String,
        use_dpi: bool,
    },
    Resize {
        item: ItemId,
        size: [String; 2],
        lock: bool,
        filter: Filter,
        original: (u32, u32),
    },
    PrintSize {
        item: ItemId,
        size: [String; 2],
        unit: PrintUnit,
        dpi: String,
        lock: bool,
        filter: Filter,
        ratio: f64,
    },
    Rotate {
        item: ItemId,
        angle: String,
    },
    Feather {
        item: ItemId,
        edges: [bool; 4],
        width: String,
        curve: Curve,
    },
    Transition {
        id: TransitionId,
        kind: TransitionKind,
        orientation: Orientation,
        width: String,
        curve: Curve,
        ratio: String,
    },
    Exif {
        items: Vec<(ItemId, String)>,
        current: ItemId,
        entries: Vec<ExifEntry>,
    },
    Export {
        settings: ExportSettings,
        quality: String,
        sources: Vec<(ItemId, String)>,
        /// The path the user confirmed in the file chooser.
        picked: PathBuf,
    },
}

#[derive(Clone, Debug)]
pub enum ModalMessage {
    Cancel,
    Ok,
    Discard,
    Strip,
    Remove,
    Text(usize, String),
    Check(usize, bool),
    Select(usize, String),
    Choice(usize),
}

fn modal(message: ModalMessage) -> Message {
    Message::Modal(message)
}

fn image_extensions() -> Vec<&'static str> {
    Format::READABLE
        .iter()
        .flat_map(|f| f.extensions().iter().copied())
        .collect()
}

/// A file dialog starting in the folder of the last file picked or saved (UI-007).
fn file_dialog(app: &App, title: &str) -> rfd::AsyncFileDialog {
    let dialog = rfd::AsyncFileDialog::new().set_title(title);
    match app.editor.dialog_folder() {
        Some(folder) => dialog.set_directory(folder),
        None => dialog,
    }
}

fn pick(
    app: &App,
    purpose: Purpose,
    title: &'static str,
    multiple: bool,
    projects: bool,
) -> Task<Message> {
    let dialog = file_dialog(app, title);
    Task::perform(
        async move {
            let mut exts = image_extensions();
            if projects {
                exts.push(project_file::EXTENSION);
            }
            let dialog = dialog.add_filter("Images", &exts);
            let files = if multiple {
                dialog.pick_files().await.unwrap_or_default()
            } else {
                dialog.pick_file().await.into_iter().collect()
            };
            files.into_iter().map(|f| f.path().to_path_buf()).collect()
        },
        move |paths| Message::Picked(purpose.clone(), paths),
    )
}

fn save_file(app: &App, purpose: Purpose, title: &'static str, name: String) -> Task<Message> {
    let dialog = file_dialog(app, title);
    Task::perform(
        async move {
            dialog
                .set_file_name(name)
                .save_file()
                .await
                .into_iter()
                .map(|f| f.path().to_path_buf())
                .collect()
        },
        move |paths| Message::Picked(purpose.clone(), paths),
    )
}

fn save_project(app: &mut App, after: AfterDiscard) -> Task<Message> {
    if app.editor.path().is_some() {
        if let Err(e) = app.editor.save() {
            app.modal = Some(Modal::Error(e));
            return Task::none();
        }
        return proceed(app, after);
    }
    let name = format!("Untitled.{}", project_file::EXTENSION);
    save_file(app, Purpose::SaveAs(after), "Save project", name)
}

fn proceed(app: &mut App, after: AfterDiscard) -> Task<Message> {
    match after {
        AfterDiscard::Nothing => Task::none(),
        AfterDiscard::Open => pick(app, Purpose::Open, "Open", true, true),
        AfterDiscard::Quit => iced::exit(),
    }
}

/// Starts the UI part of a command (dialogs and file choosers).
pub fn run(app: &mut App, command: Command) -> Task<Message> {
    let state = app.editor.project().state().clone();
    match command {
        Command::Open => {
            if app.editor.needs_save_prompt() {
                app.modal = Some(Modal::ConfirmDiscard(AfterDiscard::Open));
                Task::none()
            } else {
                pick(app, Purpose::Open, "Open", true, true)
            }
        }
        Command::AddImages => pick(app, Purpose::AddImages, "Add images", true, false),
        Command::Save => save_project(app, AfterDiscard::Nothing),
        Command::SaveAs => save_file(
            app,
            Purpose::SaveAs(AfterDiscard::Nothing),
            "Save project",
            format!("Untitled.{}", project_file::EXTENSION),
        ),
        Command::Export => {
            if app.editor.is_empty() {
                app.modal = Some(Modal::Error("There is nothing to export.".to_owned()));
                return Task::none();
            }
            let stem = app
                .editor
                .path()
                .and_then(|p| p.file_stem())
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "export".to_owned());
            save_file(app, Purpose::Export, "Export", format!("{stem}.png"))
        }
        Command::CanvasSize => {
            let (w, h) = state.canvas_size();
            app.modal = Some(Modal::CanvasSize {
                size: [
                    w.max(1).to_string(),
                    h.max(1).to_string(),
                    state.canvas.dpi.map(format_number).unwrap_or("300".into()),
                ],
                use_dpi: state.canvas.dpi.is_some(),
            });
            Task::none()
        }
        Command::SetDpi => {
            app.modal = Some(Modal::SetDpi {
                dpi: state.canvas.dpi.map(format_number).unwrap_or("300".into()),
                use_dpi: true,
            });
            Task::none()
        }
        Command::Resize(item) => {
            if let Some(i) = state.item(item) {
                app.modal = Some(Modal::Resize {
                    item,
                    size: [i.width.to_string(), i.height.to_string()],
                    lock: true,
                    filter: i.filter,
                    original: (i.width, i.height),
                });
            }
            Task::none()
        }
        Command::PrintSize(item) => {
            if let Some(i) = state.item(item) {
                let dpi = state.canvas.dpi.unwrap_or(300.0);
                app.modal = Some(Modal::PrintSize {
                    item,
                    size: [
                        format!("{:.2}", f64::from(i.width) / dpi),
                        format!("{:.2}", f64::from(i.height) / dpi),
                    ],
                    unit: PrintUnit::Inches,
                    dpi: format_number(dpi),
                    lock: true,
                    filter: i.filter,
                    ratio: f64::from(i.width) / f64::from(i.height),
                });
            }
            Task::none()
        }
        Command::RotateBy(item) => {
            let angle = state.item(item).map(|i| i.rotation).unwrap_or(0.0);
            app.modal = Some(Modal::Rotate {
                item,
                angle: format!("{angle:.1}"),
            });
            Task::none()
        }
        Command::Feather(item) => {
            let f = app.editor.default_feather(item);
            app.modal = Some(Modal::Feather {
                item,
                edges: [f.left, f.right, f.top, f.bottom],
                width: format_number(f.width),
                curve: f.curve,
            });
            Task::none()
        }
        Command::TransitionProperties(id) => {
            if let Some(t) = state.transition(id) {
                let (orientation, width, curve, ratio) = match t.kind {
                    TransitionKind::GradientSeam {
                        orientation,
                        width,
                        curve,
                        ..
                    } => (orientation, width, curve, 0.5),
                    TransitionKind::Crossfade { ratio } => {
                        (Orientation::Horizontal, 10.0, Curve::Linear, ratio)
                    }
                };
                app.modal = Some(Modal::Transition {
                    id,
                    kind: t.kind,
                    orientation,
                    width: format_number(width),
                    curve,
                    ratio: format_number(ratio * 100.0),
                });
            }
            Task::none()
        }
        Command::ExifViewer(item) => {
            let items: Vec<(ItemId, String)> = app
                .editor
                .items_with_exif()
                .into_iter()
                .map(|i| (i, app.editor.item_name(i)))
                .collect();
            if items.is_empty() {
                app.modal = Some(Modal::Error("No image has EXIF metadata.".to_owned()));
                return Task::none();
            }
            let current = item
                .or_else(|| app.editor.exif_viewer_item())
                .filter(|i| items.iter().any(|(id, _)| id == i))
                .unwrap_or(items[0].0);
            let entries = app.editor.exif_entries(current);
            app.modal = Some(Modal::Exif {
                items,
                current,
                entries,
            });
            Task::none()
        }
        Command::LocateSource(source) => {
            pick(app, Purpose::Locate(source), "Locate image", false, false)
        }
        other => {
            app.editor.execute(other);
            Task::none()
        }
    }
}

/// Handles the result of a file chooser.
pub fn picked(app: &mut App, purpose: Purpose, paths: Vec<PathBuf>) -> Task<Message> {
    let Some(first) = paths.first().cloned() else {
        return Task::none();
    };
    app.editor.remember_folder(&first);
    let result = match purpose {
        Purpose::Open if paths.len() == 1 => app.editor.open(&first),
        Purpose::Open => {
            app.modal = Some(Modal::CanvasChoice {
                paths,
                choice: 0,
                size: ["1920".into(), "1080".into(), "300".into()],
                use_dpi: false,
            });
            Ok(())
        }
        Purpose::AddImages => app.editor.add_images(&paths, None),
        Purpose::SaveAs(after) => match app.editor.save_as(&first) {
            Ok(()) => return proceed(app, after),
            Err(e) => Err(e),
        },
        Purpose::Export => {
            if app.editor.export_overwrites_source(&first) {
                app.modal = Some(Modal::ConfirmOverwrite(first));
            } else {
                open_export(app, first);
            }
            Ok(())
        }
        Purpose::Locate(source) => {
            app.editor.relink(source, &first);
            Ok(())
        }
    };
    if let Err(e) = result {
        app.modal = Some(Modal::Error(e));
    }
    Task::none()
}

fn open_export(app: &mut App, path: PathBuf) {
    let settings = app.editor.export_defaults(&path);
    let sources = app
        .editor
        .items_with_exif()
        .into_iter()
        .map(|i| (i, app.editor.item_name(i)))
        .collect();
    app.modal = Some(Modal::Export {
        quality: settings.quality.to_string(),
        settings,
        sources,
        picked: path,
    });
}

fn canvas_choices() -> Vec<CanvasChoice> {
    let mut choices = vec![
        CanvasChoice::SideBySide,
        CanvasChoice::Stacked,
        CanvasChoice::Largest,
    ];
    choices.extend(Preset::ALL.map(CanvasChoice::Preset));
    choices
}

fn num(text: &str) -> Option<f64> {
    text.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Updates the open dialog; `Ok` applies it.
pub fn update(app: &mut App, message: ModalMessage) -> Task<Message> {
    let Some(mut current) = app.modal.take() else {
        return Task::none();
    };
    match message {
        ModalMessage::Cancel => {
            if let Modal::ConfirmDiscard(AfterDiscard::Quit) = current {
                return Task::none();
            }
            return Task::none();
        }
        ModalMessage::Discard => {
            if let Modal::ConfirmDiscard(after) = current {
                return proceed(app, after);
            }
        }
        ModalMessage::Ok => return apply(app, current),
        ModalMessage::Strip => {
            if let Modal::Exif { current: item, .. } = current {
                app.editor.strip_metadata(item);
                return Task::none();
            }
        }
        ModalMessage::Remove => {
            if let Modal::Transition { id, .. } = current {
                app.editor.execute(Command::RemoveTransition(id));
                return Task::none();
            }
        }
        ModalMessage::Text(field, value) => edit_text(&mut current, field, value),
        ModalMessage::Check(field, value) => edit_check(&mut current, field, value),
        ModalMessage::Select(field, value) => edit_select(app, &mut current, field, value),
        ModalMessage::Choice(index) => {
            if let Modal::CanvasChoice { choice, .. } = &mut current {
                *choice = index;
            }
        }
    }
    app.modal = Some(current);
    Task::none()
}

fn edit_text(modal: &mut Modal, field: usize, value: String) {
    match modal {
        Modal::CanvasChoice { size, .. } | Modal::CanvasSize { size, .. } => size[field] = value,
        Modal::SetDpi { dpi, .. } => *dpi = value,
        Modal::Resize {
            size,
            lock,
            original,
            ..
        } => {
            if *lock && let Ok(v) = value.trim().parse::<u32>() {
                let other = if field == 0 {
                    proportional(original.0, original.1, v.max(1))
                } else {
                    proportional(original.1, original.0, v.max(1))
                };
                size[1 - field] = other.to_string();
            }
            size[field] = value;
        }
        Modal::PrintSize {
            size,
            lock,
            ratio,
            dpi,
            ..
        } => {
            if field == 2 {
                *dpi = value;
                return;
            }
            if *lock && let Some(v) = num(&value) {
                let other = if field == 0 { v / *ratio } else { v * *ratio };
                size[1 - field] = format!("{other:.2}");
            }
            size[field] = value;
        }
        Modal::Rotate { angle, .. } => *angle = value,
        Modal::Feather { width, .. } => *width = value,
        Modal::Transition { width, ratio, .. } => {
            if field == 0 {
                *width = value;
            } else {
                *ratio = value;
            }
        }
        Modal::Export { quality, .. } => *quality = value,
        _ => {}
    }
}

fn edit_check(modal: &mut Modal, field: usize, value: bool) {
    match modal {
        Modal::CanvasChoice { use_dpi, .. }
        | Modal::CanvasSize { use_dpi, .. }
        | Modal::SetDpi { use_dpi, .. } => *use_dpi = value,
        Modal::Resize { lock, .. } | Modal::PrintSize { lock, .. } => *lock = value,
        Modal::Feather { edges, .. } => edges[field] = value,
        Modal::Export { settings, .. } => {
            let group = ExifGroup::ALL[field];
            settings.groups.retain(|g| *g != group);
            if value {
                settings.groups.push(group);
                settings.groups.sort();
            }
        }
        _ => {}
    }
}

fn edit_select(app: &mut App, modal: &mut Modal, field: usize, value: String) {
    match modal {
        Modal::Resize { filter, .. } | Modal::PrintSize { filter, .. } if field == 0 => {
            if let Some(f) = Filter::ALL.into_iter().find(|f| f.label() == value) {
                *filter = f;
            }
        }
        Modal::PrintSize { unit, size, .. } if field == 1 => {
            let new = if value == "cm" {
                PrintUnit::Centimeters
            } else {
                PrintUnit::Inches
            };
            if new != *unit {
                let factor = if new == PrintUnit::Centimeters {
                    2.54
                } else {
                    1.0 / 2.54
                };
                for s in size.iter_mut() {
                    if let Some(v) = num(s) {
                        *s = format!("{:.2}", v * factor);
                    }
                }
                *unit = new;
            }
        }
        Modal::Feather { curve, .. } | Modal::Transition { curve, .. } if field == 0 => {
            if let Some(c) = Curve::ALL.into_iter().find(|c| c.label() == value) {
                *curve = c;
            }
        }
        Modal::Transition { orientation, .. } if field == 1 => {
            *orientation = if value == "Vertical" {
                Orientation::Vertical
            } else {
                Orientation::Horizontal
            };
        }
        Modal::Exif {
            items,
            current,
            entries,
        } => {
            if let Some((id, _)) = items.iter().find(|(_, name)| *name == value) {
                *current = *id;
                *entries = app.editor.exif_entries(*id);
            }
        }
        Modal::Export {
            settings, sources, ..
        } => {
            if field == 0 {
                if let Some(f) = Format::WRITABLE.into_iter().find(|f| f.label() == value) {
                    settings.format = f;
                    if Format::from_path(&settings.path) != Some(f) {
                        settings.path.set_extension(f.extensions()[0]);
                    }
                }
            } else {
                settings.metadata = sources
                    .iter()
                    .find(|(_, name)| *name == value)
                    .map(|(id, _)| *id);
            }
        }
        _ => {}
    }
}

/// Applies a dialog. Invalid input keeps the dialog open.
fn apply(app: &mut App, current: Modal) -> Task<Message> {
    let editor = &mut app.editor;
    let invalid = |app: &mut App, current: Modal| {
        app.modal = Some(current);
        Task::none()
    };
    match &current {
        Modal::Error(_) => {}
        Modal::ConfirmDiscard(after) => return save_project(app, *after),
        Modal::ConfirmOverwrite(path) => {
            open_export(app, path.clone());
            return Task::none();
        }
        Modal::CanvasChoice {
            paths,
            choice,
            size,
            use_dpi,
        } => {
            let choices = canvas_choices();
            let choice = match choices.get(*choice) {
                Some(c) => *c,
                None => {
                    let (Some(w), Some(h)) = (num(&size[0]), num(&size[1])) else {
                        return invalid(app, current);
                    };
                    CanvasChoice::Custom {
                        width: w.max(1.0) as u32,
                        height: h.max(1.0) as u32,
                        dpi: if *use_dpi { num(&size[2]) } else { None },
                    }
                }
            };
            if let Err(e) = editor.new_composition(paths, Some(choice)) {
                app.modal = Some(Modal::Error(e));
            }
        }
        Modal::CanvasSize { size, use_dpi } => {
            let (Some(w), Some(h)) = (num(&size[0]), num(&size[1])) else {
                return invalid(app, current);
            };
            let dpi = if *use_dpi { num(&size[2]) } else { None };
            editor.set_canvas_size(w.max(1.0) as u32, h.max(1.0) as u32, dpi);
        }
        Modal::SetDpi { dpi, use_dpi } => {
            let dpi = if *use_dpi { num(dpi) } else { None };
            editor.set_dpi(dpi);
        }
        Modal::Resize {
            item, size, filter, ..
        } => {
            let (Some(w), Some(h)) = (num(&size[0]), num(&size[1])) else {
                return invalid(app, current);
            };
            editor.resize_item(*item, w.max(1.0) as u32, h.max(1.0) as u32, *filter);
        }
        Modal::PrintSize {
            item,
            size,
            unit,
            dpi,
            filter,
            ..
        } => {
            let (Some(w), Some(h), Some(d)) = (num(&size[0]), num(&size[1]), num(dpi)) else {
                return invalid(app, current);
            };
            editor.set_print_size(*item, w, h, *unit, d, *filter);
        }
        Modal::Rotate { item, angle } => {
            let Some(a) = num(angle) else {
                return invalid(app, current);
            };
            editor.rotate_to(*item, a);
        }
        Modal::Feather {
            item,
            edges,
            width,
            curve,
        } => {
            let Some(w) = num(width) else {
                return invalid(app, current);
            };
            let feather = Feather {
                left: edges[0],
                right: edges[1],
                top: edges[2],
                bottom: edges[3],
                width: w,
                curve: *curve,
            };
            editor.set_feather(*item, feather.any_edge().then_some(feather));
        }
        Modal::Transition {
            id,
            kind,
            orientation,
            width,
            curve,
            ratio,
        } => {
            let kind = match kind {
                TransitionKind::GradientSeam { offset, .. } => {
                    let Some(w) = num(width) else {
                        return invalid(app, current);
                    };
                    TransitionKind::GradientSeam {
                        orientation: *orientation,
                        offset: *offset,
                        width: w,
                        curve: *curve,
                    }
                }
                TransitionKind::Crossfade { .. } => {
                    let Some(r) = num(ratio) else {
                        return invalid(app, current);
                    };
                    TransitionKind::Crossfade {
                        ratio: (r / 100.0).clamp(0.0, 1.0),
                    }
                }
            };
            editor.edit_transition(*id, kind);
        }
        Modal::Exif { .. } => {}
        Modal::Export {
            settings,
            quality,
            picked,
            ..
        } => {
            let mut settings = settings.clone();
            let Some(q) = num(quality) else {
                return invalid(app, current);
            };
            if Format::from_path(&settings.path) != Some(settings.format) {
                settings.path.set_extension(settings.format.extensions()[0]);
            }
            // A changed extension is a file the user has not confirmed (EXPORT-003).
            if settings.path != *picked
                && (editor.export_overwrites_source(&settings.path) || settings.path.exists())
            {
                app.modal = Some(Modal::Error(format!(
                    "{} already exists. Export again and choose that name to replace it.",
                    settings.path.display()
                )));
                return Task::none();
            }
            settings.quality = q.clamp(1.0, 100.0) as u8;
            if let Err(e) = editor.export(&settings) {
                app.modal = Some(Modal::Error(e));
            }
        }
    }
    Task::none()
}

fn labeled<'a>(label: &'a str, content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    row![text(label).width(180), content.into()]
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
}

fn input<'a>(value: &'a str, field: usize) -> Element<'a, Message> {
    text_input("", value)
        .on_input(move |v| modal(ModalMessage::Text(field, v)))
        .on_submit(modal(ModalMessage::Ok))
        .width(120)
        .into()
}

fn check<'a>(label: &'a str, value: bool, field: usize) -> Element<'a, Message> {
    checkbox(value)
        .label(label)
        .on_toggle(move |v| modal(ModalMessage::Check(field, v)))
        .into()
}

fn select<'a>(options: Vec<String>, selected: &str, field: usize) -> Element<'a, Message> {
    pick_list(options, Some(selected.to_owned()), move |v| {
        modal(ModalMessage::Select(field, v))
    })
    .into()
}

fn card<'a>(
    title: &'a str,
    body: Element<'a, Message>,
    buttons: Vec<Element<'a, Message>>,
) -> Element<'a, Message> {
    let mut actions = row![Space::new().width(Length::Fill)].spacing(8);
    for b in buttons {
        actions = actions.push(b);
    }
    container(
        column![text(title).size(20), body, actions]
            .spacing(16)
            .width(520),
    )
    .padding(20)
    .style(container::bordered_box)
    .into()
}

fn ok_cancel<'a>(ok: &'a str) -> Vec<Element<'a, Message>> {
    vec![
        button(text("Cancel"))
            .style(button::secondary)
            .on_press(modal(ModalMessage::Cancel))
            .into(),
        button(text(ok)).on_press(modal(ModalMessage::Ok)).into(),
    ]
}

/// The open dialog.
pub fn view<'a>(_app: &'a App, current: &'a Modal) -> Element<'a, Message> {
    let filters = || Filter::ALL.map(|f| f.label().to_owned()).to_vec();
    let curves = || Curve::ALL.map(|c| c.label().to_owned()).to_vec();
    match current {
        Modal::Error(message) => card(
            "Something went wrong",
            text(message.as_str()).into(),
            vec![
                button(text("OK"))
                    .on_press(modal(ModalMessage::Cancel))
                    .into(),
            ],
        ),
        Modal::ConfirmDiscard(_) => card(
            "Save changes?",
            text("The project has unsaved changes.").into(),
            vec![
                button(text("Cancel"))
                    .style(button::secondary)
                    .on_press(modal(ModalMessage::Cancel))
                    .into(),
                button(text("Discard"))
                    .style(button::danger)
                    .on_press(modal(ModalMessage::Discard))
                    .into(),
                button(text("Save"))
                    .on_press(modal(ModalMessage::Ok))
                    .into(),
            ],
        ),
        Modal::ConfirmOverwrite(path) => card(
            "Overwrite a source image?",
            text(format!(
                "{} is used by this project. Exporting will replace it.",
                path.display()
            ))
            .into(),
            ok_cancel("Overwrite"),
        ),
        Modal::CanvasChoice {
            choice,
            size,
            use_dpi,
            ..
        } => {
            let choices = canvas_choices();
            let mut list = column![].spacing(6);
            for (i, c) in choices.iter().enumerate() {
                list = list.push(radio(c.label(), i, Some(*choice), |i| {
                    modal(ModalMessage::Choice(i))
                }));
            }
            let custom = choices.len();
            list = list.push(radio("Custom", custom, Some(*choice), |i| {
                modal(ModalMessage::Choice(i))
            }));
            list = list.push(
                row![
                    input(&size[0], 0),
                    text("×"),
                    input(&size[1], 1),
                    text("px"),
                    check("DPI", *use_dpi, 0),
                    input(&size[2], 2)
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            );
            card(
                "New composition — canvas size",
                list.into(),
                ok_cancel("Create"),
            )
        }
        Modal::CanvasSize { size, use_dpi } => card(
            "Canvas size",
            column![
                labeled("Width (px)", input(&size[0], 0)),
                labeled("Height (px)", input(&size[1], 1)),
                labeled(
                    "DPI",
                    row![check("Set", *use_dpi, 0), input(&size[2], 2)]
                        .spacing(8)
                        .align_y(Alignment::Center)
                ),
            ]
            .spacing(10)
            .into(),
            ok_cancel("Apply"),
        ),
        Modal::SetDpi { dpi, use_dpi } => card(
            "Set DPI",
            column![check("Set DPI", *use_dpi, 0), labeled("DPI", input(dpi, 0))]
                .spacing(10)
                .into(),
            ok_cancel("Apply"),
        ),
        Modal::Resize {
            size, lock, filter, ..
        } => card(
            "Resize image",
            column![
                labeled("Width (px)", input(&size[0], 0)),
                labeled("Height (px)", input(&size[1], 1)),
                check("Keep aspect ratio", *lock, 0),
                labeled("Resampling", select(filters(), filter.label(), 0)),
            ]
            .spacing(10)
            .into(),
            ok_cancel("Resize"),
        ),
        Modal::PrintSize {
            size,
            unit,
            dpi,
            lock,
            filter,
            ..
        } => card(
            "Print size",
            column![
                labeled(
                    "Unit",
                    select(vec!["in".into(), "cm".into()], unit.label(), 1)
                ),
                labeled("Width", input(&size[0], 0)),
                labeled("Height", input(&size[1], 1)),
                check("Keep aspect ratio", *lock, 0),
                labeled("DPI", input(dpi, 2)),
                labeled("Resampling", select(filters(), filter.label(), 0)),
            ]
            .spacing(10)
            .into(),
            ok_cancel("Apply"),
        ),
        Modal::Rotate { angle, .. } => card(
            "Rotate",
            labeled("Angle (degrees, clockwise)", input(angle, 0)),
            ok_cancel("Rotate"),
        ),
        Modal::Feather {
            edges,
            width,
            curve,
            ..
        } => card(
            "Feather edges",
            column![
                labeled(
                    "Edges",
                    row![
                        check("Left", edges[0], 0),
                        check("Right", edges[1], 1),
                        check("Top", edges[2], 2),
                        check("Bottom", edges[3], 3)
                    ]
                    .spacing(10)
                ),
                labeled("Width (px)", input(width, 0)),
                labeled("Curve", select(curves(), curve.label(), 0)),
            ]
            .spacing(10)
            .into(),
            ok_cancel("Apply"),
        ),
        Modal::Transition {
            kind,
            orientation,
            width,
            curve,
            ratio,
            ..
        } => {
            let body: Element<'_, Message> = match kind {
                TransitionKind::GradientSeam { .. } => column![
                    labeled(
                        "Orientation",
                        select(
                            vec!["Horizontal".into(), "Vertical".into()],
                            orientation.label(),
                            1
                        )
                    ),
                    labeled("Band width (px)", input(width, 0)),
                    labeled("Curve", select(curves(), curve.label(), 0)),
                ]
                .spacing(10)
                .into(),
                TransitionKind::Crossfade { .. } => {
                    labeled("Upper image opacity (%)", input(ratio, 1))
                }
            };
            let mut buttons = vec![
                button(text("Remove transition"))
                    .style(button::danger)
                    .on_press(modal(ModalMessage::Remove))
                    .into(),
            ];
            buttons.extend(ok_cancel("Apply"));
            card(kind.label(), body, buttons)
        }
        Modal::Exif {
            items,
            current,
            entries,
        } => {
            let mut table = column![].spacing(4);
            let mut group = None;
            for entry in entries {
                if group != Some(entry.group) {
                    group = Some(entry.group);
                    table = table.push(text(entry.group.label()).size(16));
                }
                table = table.push(
                    row![
                        text(entry.name.as_str()).width(240).size(13),
                        text(entry.value.as_str()).size(13)
                    ]
                    .spacing(12),
                );
            }
            let name = items
                .iter()
                .find(|(id, _)| id == current)
                .map(|(_, n)| n.as_str())
                .unwrap_or_default();
            let mut body = column![].spacing(10);
            if items.len() > 1 {
                body = body.push(select(
                    items.iter().map(|(_, n)| n.clone()).collect(),
                    name,
                    0,
                ));
            }
            body = body.push(scrollable(table).height(380));
            card(
                "EXIF metadata",
                body.into(),
                vec![
                    button(text("Strip metadata"))
                        .style(button::danger)
                        .on_press(modal(ModalMessage::Strip))
                        .into(),
                    button(text("Close"))
                        .on_press(modal(ModalMessage::Cancel))
                        .into(),
                ],
            )
        }
        Modal::Export {
            settings,
            quality,
            sources,
            ..
        } => {
            let formats = Format::WRITABLE.map(|f| f.label().to_owned()).to_vec();
            let mut names = vec!["None".to_owned()];
            names.extend(sources.iter().map(|(_, n)| n.clone()));
            let source = settings
                .metadata
                .and_then(|m| sources.iter().find(|(id, _)| *id == m))
                .map(|(_, n)| n.as_str())
                .unwrap_or("None");
            let mut groups = column![].spacing(4);
            if settings.metadata.is_some() {
                for (i, g) in ExifGroup::ALL.iter().enumerate() {
                    groups = groups.push(check(g.label(), settings.groups.contains(g), i));
                }
            }
            let mut body = column![
                text(settings.path.display().to_string()).size(13),
                labeled("Format", select(formats, settings.format.label(), 0)),
            ]
            .spacing(10);
            if settings.format.is_lossy() {
                body = body.push(labeled("Quality", input(quality, 0)));
            }
            body = body.push(labeled("EXIF metadata from", select(names, source, 1)));
            body = body.push(groups);
            card("Export options", body.into(), ok_cancel("Export"))
        }
    }
}

/// Status bar: canvas size and zoom, missing sources, EXIF notice.
pub fn status(app: &App) -> Element<'_, Message> {
    // UI-010: the zoom level opens the zoom menu.
    let zoom = button(text(app.editor.zoom_text()).size(13))
        .style(button::text)
        .padding([0, 4])
        .on_press(Message::ZoomMenu);
    let mut bar = row![text(app.editor.canvas_text()).size(13), zoom]
        .spacing(12)
        .padding([2, 8])
        .align_y(Alignment::Center);
    bar = bar.push(Space::new().width(Length::Fill));
    if let Some(missing) = &app.missing_notice {
        bar = bar.push(text(missing.as_str()).size(13));
        bar = bar.push(
            button(text("Locate…").size(13))
                .style(button::text)
                .on_press(Message::Locate),
        );
    }
    if let Some(notice) = &app.exif_notice {
        bar = bar.push(
            button(text(format!("ⓘ {notice}")).size(13))
                .style(button::text)
                .on_press(Message::Exif),
        );
    }
    bar.into()
}
