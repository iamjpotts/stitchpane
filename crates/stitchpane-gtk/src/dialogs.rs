//! Dialogs and menus for commands the editor cannot complete alone.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    Align, Box as GBox, Button, CheckButton, DropDown, FileDialog, FileFilter, Grid, Label,
    Orientation as GOrientation, Popover, ScrolledWindow, Separator, SpinButton, Window, gdk, gio,
    glib,
};
use stitchpane_core::codec::Format;
use stitchpane_core::editor::{CanvasChoice, Command, MenuEntry, Preset, PrintUnit, proportional};
use stitchpane_core::metadata::ExifGroup;
use stitchpane_core::model::{
    Curve, Feather, Filter, ItemId, Orientation, SourceId, TransitionId, TransitionKind,
};
use stitchpane_core::project_file;

use crate::Ui;

pub(crate) fn run(ui: &Rc<Ui>, command: Command) {
    match command {
        Command::Open => open(ui),
        Command::AddImages => add_images(ui),
        Command::Save => {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                save(&ui).await;
            });
        }
        Command::SaveAs => {
            let ui = ui.clone();
            glib::spawn_future_local(async move {
                save_as(&ui).await;
            });
        }
        Command::Export => export(ui),
        Command::CanvasSize => canvas_size(ui),
        Command::SetDpi => set_dpi(ui),
        Command::Resize(item) => resize(ui, item),
        Command::PrintSize(item) => print_size(ui, item),
        Command::RotateBy(item) => rotate(ui, item),
        Command::Feather(item) => feather(ui, item),
        Command::TransitionProperties(id) => transition(ui, id),
        Command::ExifViewer(item) => exif_viewer(ui, item),
        Command::LocateSource(source) => locate(ui, source),
        // closing the window offers to save first (UI-002, UI-013)
        Command::Quit => ui.window.close(),
        other => {
            ui.edit(|e| e.execute(other));
            ui.refresh();
        }
    }
}

/// Context menu at a canvas position (UI-004).
pub(crate) fn context_menu(ui: &Rc<Ui>, entries: Vec<MenuEntry>, x: f64, y: f64) {
    let popover = Popover::new();
    popover.set_parent(&ui.canvas);
    popover.set_has_arrow(false);
    popover.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
    popover.set_halign(Align::Start);
    let list = GBox::new(GOrientation::Vertical, 0);
    for entry in entries {
        match entry.command {
            None => list.append(&Separator::new(GOrientation::Horizontal)),
            Some(command) => {
                let button = Button::with_label(&entry.label);
                button.add_css_class("flat");
                if let Some(label) = button.child() {
                    label.set_halign(Align::Start);
                }
                let weak = Rc::downgrade(ui);
                let popover_ref = popover.clone();
                button.connect_clicked(move |_| {
                    popover_ref.popdown();
                    if let Some(ui) = weak.upgrade() {
                        ui.command(command.clone());
                    }
                });
                list.append(&button);
            }
        }
    }
    popover.set_child(Some(&list));
    popover.connect_closed(|p| {
        let p = p.clone();
        glib::idle_add_local_once(move || p.unparent());
    });
    popover.popup();
}

fn form(rows: &[(&str, &gtk4::Widget)]) -> Grid {
    let grid = Grid::builder().row_spacing(8).column_spacing(12).build();
    for (i, (label, widget)) in rows.iter().enumerate() {
        let l = Label::new(Some(label));
        l.set_halign(Align::End);
        grid.attach(&l, 0, i as i32, 1, 1);
        grid.attach(*widget, 1, i as i32, 1, 1);
    }
    grid
}

/// A modal dialog with Cancel and an action button. `on_ok` returns whether to close.
fn dialog(
    ui: &Rc<Ui>,
    title: &str,
    body: &impl IsA<gtk4::Widget>,
    ok_label: &str,
    on_ok: impl Fn(&Rc<Ui>) -> bool + 'static,
) -> Window {
    let window = Window::builder()
        .title(title)
        .modal(true)
        .transient_for(&ui.window)
        .resizable(false)
        .build();
    let content = GBox::new(GOrientation::Vertical, 12);
    for m in [
        Box::new(|w: &GBox| w.set_margin_top(18)) as Box<dyn Fn(&GBox)>,
        Box::new(|w: &GBox| w.set_margin_bottom(18)),
        Box::new(|w: &GBox| w.set_margin_start(18)),
        Box::new(|w: &GBox| w.set_margin_end(18)),
    ] {
        m(&content);
    }
    content.append(body);
    let buttons = GBox::new(GOrientation::Horizontal, 8);
    buttons.set_halign(Align::End);
    let cancel = Button::with_label("Cancel");
    let ok = Button::with_label(ok_label);
    ok.add_css_class("suggested-action");
    buttons.append(&cancel);
    buttons.append(&ok);
    content.append(&buttons);
    window.set_child(Some(&content));
    let w = window.clone();
    cancel.connect_clicked(move |_| w.close());
    let w = window.clone();
    let weak = Rc::downgrade(ui);
    ok.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            if on_ok(&ui) {
                w.close();
            }
            ui.refresh();
        }
    });
    let keys = gtk4::EventControllerKey::new();
    let w = window.clone();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key == gdk::Key::Escape {
            w.close();
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(keys);
    window.set_default_widget(Some(&ok));
    window.present();
    window
}

fn spin(min: f64, max: f64, step: f64, digits: u32, value: f64) -> SpinButton {
    let s = SpinButton::with_range(min, max, step);
    s.set_digits(digits);
    s.set_value(value);
    s.set_activates_default(true);
    s
}

fn dropdown(items: &[&str], selected: usize) -> DropDown {
    let d = DropDown::from_strings(items);
    d.set_selected(selected as u32);
    d
}

fn filter_dropdown(current: Filter) -> DropDown {
    let labels: Vec<&str> = Filter::ALL.iter().map(|f| f.label()).collect();
    let index = Filter::ALL.iter().position(|f| *f == current).unwrap_or(2);
    dropdown(&labels, index)
}

fn curve_dropdown(current: Curve) -> DropDown {
    let labels: Vec<&str> = Curve::ALL.iter().map(|c| c.label()).collect();
    let index = Curve::ALL.iter().position(|c| *c == current).unwrap_or(0);
    dropdown(&labels, index)
}

fn image_filter() -> FileFilter {
    let filter = FileFilter::new();
    filter.set_name(Some("Images"));
    for format in Format::READABLE {
        for ext in format.extensions() {
            filter.add_suffix(ext);
        }
    }
    filter
}

fn project_filter() -> FileFilter {
    let filter = FileFilter::new();
    filter.set_name(Some("Stitch Pane projects"));
    filter.add_suffix(project_file::EXTENSION);
    filter
}

fn filters(list: &[FileFilter]) -> gio::ListStore {
    let store = gio::ListStore::new::<FileFilter>();
    for f in list {
        store.append(f);
    }
    store
}

/// UI-007: file dialogs start in the folder of the last file picked or saved.
fn start_in_last_folder(ui: &Rc<Ui>, dialog: &FileDialog) {
    if let Some(folder) = ui
        .read(|e| e.dialog_folder().map(Path::to_path_buf))
        .flatten()
    {
        dialog.set_initial_folder(Some(&gio::File::for_path(folder)));
    }
}

/// The picked file's path, remembered for the next file dialog (UI-007).
fn picked(ui: &Rc<Ui>, file: &gio::File) -> Option<PathBuf> {
    let path = file.path()?;
    ui.edit(|e| e.remember_folder(&path));
    Some(path)
}

async fn pick_files(ui: &Rc<Ui>, title: &str, with_projects: bool) -> Vec<PathBuf> {
    let all = image_filter();
    if with_projects {
        all.set_name(Some("Images and projects"));
        all.add_suffix(project_file::EXTENSION);
    }
    let mut list = vec![all];
    if with_projects {
        list.push(project_filter());
    }
    list.push(image_filter());
    let dialog = FileDialog::builder()
        .title(title)
        .modal(true)
        .filters(&filters(&list))
        .build();
    start_in_last_folder(ui, &dialog);
    let Ok(files) = dialog.open_multiple_future(Some(&ui.window)).await else {
        return Vec::new();
    };
    (0..files.n_items())
        .filter_map(|i| match files.item(i)?.downcast::<gio::File>() {
            Ok(file) => picked(ui, &file),
            // the dialog only returns files
            Err(_) => None,
        })
        .collect()
}

/// UI-002: asks whether to save unsaved changes. Returns true to go ahead.
pub(crate) async fn confirm_discard(ui: &Rc<Ui>) -> bool {
    // if the editor is busy, ask rather than risk losing work
    if !ui.read(|e| e.needs_save_prompt()).unwrap_or(true) {
        return true;
    }
    let choice = gtk4::AlertDialog::builder()
        .message("Save changes?")
        .detail("The project has unsaved changes.")
        .buttons(["Cancel", "Discard", "Save"])
        .cancel_button(0)
        .default_button(2)
        .modal(true)
        .build()
        .choose_future(Some(&ui.window))
        .await;
    match choice {
        Ok(1) => true,
        Ok(2) => save(ui).await,
        _ => false,
    }
}

async fn save(ui: &Rc<Ui>) -> bool {
    let Some(untitled) = ui.read(|e| e.path().is_none()) else {
        return false;
    };
    if untitled {
        return save_as(ui).await;
    }
    let Some(result) = ui.edit(|e| e.save()) else {
        return false;
    };
    ui.refresh();
    match result {
        Ok(()) => true,
        Err(e) => {
            ui.error(&e);
            false
        }
    }
}

async fn save_as(ui: &Rc<Ui>) -> bool {
    let name = ui
        .read(|e| {
            e.path()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
        })
        .flatten()
        .unwrap_or_else(|| format!("Untitled.{}", project_file::EXTENSION));
    let dialog = FileDialog::builder()
        .title("Save project")
        .modal(true)
        .initial_name(name)
        .filters(&filters(&[project_filter()]))
        .build();
    start_in_last_folder(ui, &dialog);
    let Ok(file) = dialog.save_future(Some(&ui.window)).await else {
        return false;
    };
    let Some(path) = picked(ui, &file) else {
        return false;
    };
    let Some(result) = ui.edit(|e| e.save_as(&path)) else {
        return false;
    };
    ui.refresh();
    match result {
        Ok(()) => true,
        Err(e) => {
            ui.error(&e);
            false
        }
    }
}

/// UI-001: open images or a project.
fn open(ui: &Rc<Ui>) {
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        if !confirm_discard(&ui).await {
            return;
        }
        let paths = pick_files(&ui, "Open", true).await;
        match paths.as_slice() {
            [] => {}
            [path] => {
                if let Some(Err(e)) = ui.edit(|e| e.open(path)) {
                    ui.error(&e);
                }
                ui.refresh();
            }
            _ => canvas_choice(&ui, paths),
        }
    });
}

/// PLACE-001: add images to the project.
fn add_images(ui: &Rc<Ui>) {
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let paths = pick_files(&ui, "Add images", false).await;
        if paths.is_empty() {
            return;
        }
        if let Some(Err(e)) = ui.edit(|e| e.add_images(&paths, None)) {
            ui.error(&e);
        }
        ui.refresh();
    });
}

/// CANVAS-001: choose the canvas for a new composition.
fn canvas_choice(ui: &Rc<Ui>, paths: Vec<PathBuf>) {
    let mut choices: Vec<CanvasChoice> = vec![
        CanvasChoice::SideBySide,
        CanvasChoice::Stacked,
        CanvasChoice::Largest,
    ];
    choices.extend(Preset::ALL.map(CanvasChoice::Preset));
    let body = GBox::new(GOrientation::Vertical, 6);
    body.append(&Label::new(Some("Canvas size")));
    let mut radios: Vec<CheckButton> = Vec::new();
    for choice in &choices {
        let radio = CheckButton::with_label(&choice.label());
        if let Some(first) = radios.first() {
            radio.set_group(Some(first));
        }
        body.append(&radio);
        radios.push(radio);
    }
    let custom = CheckButton::with_label("Custom");
    custom.set_group(radios.first());
    if let Some(first) = radios.first() {
        first.set_active(true);
    }
    body.append(&custom);
    let width = spin(1.0, 100_000.0, 1.0, 0, 1920.0);
    let height = spin(1.0, 100_000.0, 1.0, 0, 1080.0);
    let use_dpi = CheckButton::with_label("DPI");
    let dpi = spin(1.0, 10_000.0, 1.0, 0, 300.0);
    let custom_row = GBox::new(GOrientation::Horizontal, 6);
    custom_row.set_margin_start(24);
    custom_row.append(&width);
    custom_row.append(&Label::new(Some("×")));
    custom_row.append(&height);
    custom_row.append(&Label::new(Some("px")));
    custom_row.append(&use_dpi);
    custom_row.append(&dpi);
    body.append(&custom_row);
    dialog(ui, "New composition", &body, "Create", move |ui| {
        let choice = if custom.is_active() {
            CanvasChoice::Custom {
                width: width.value() as u32,
                height: height.value() as u32,
                dpi: use_dpi.is_active().then(|| dpi.value()),
            }
        } else {
            let index = radios.iter().position(|r| r.is_active()).unwrap_or(0);
            let Some(&choice) = choices.get(index) else {
                return true;
            };
            choice
        };
        if let Some(Err(e)) = ui.edit(|e| e.new_composition(&paths, Some(choice))) {
            ui.error(&e);
        }
        true
    });
}

/// CANVAS-002.
fn canvas_size(ui: &Rc<Ui>) {
    let Some((w, h, current_dpi)) = ui.read(|editor| {
        let state = editor.project().state();
        let (w, h) = state.canvas_size();
        (w, h, state.canvas.dpi)
    }) else {
        return;
    };
    let width = spin(1.0, 100_000.0, 1.0, 0, f64::from(w.max(1)));
    let height = spin(1.0, 100_000.0, 1.0, 0, f64::from(h.max(1)));
    let use_dpi = CheckButton::with_label("Set");
    use_dpi.set_active(current_dpi.is_some());
    let dpi = spin(1.0, 10_000.0, 1.0, 0, current_dpi.unwrap_or(300.0));
    let dpi_row = GBox::new(GOrientation::Horizontal, 6);
    dpi_row.append(&use_dpi);
    dpi_row.append(&dpi);
    let body = form(&[
        ("Width (px)", width.upcast_ref()),
        ("Height (px)", height.upcast_ref()),
        ("DPI", dpi_row.upcast_ref()),
    ]);
    dialog(ui, "Canvas size", &body, "Apply", move |ui| {
        ui.edit(|e| {
            e.set_canvas_size(
                width.value() as u32,
                height.value() as u32,
                use_dpi.is_active().then(|| dpi.value()),
            );
        });
        true
    });
}

/// RES-003.
fn set_dpi(ui: &Rc<Ui>) {
    let Some(current) = ui.read(|e| e.project().state().canvas.dpi) else {
        return;
    };
    let use_dpi = CheckButton::with_label("Set DPI");
    use_dpi.set_active(true);
    let dpi = spin(1.0, 10_000.0, 1.0, 0, current.unwrap_or(300.0));
    let body = form(&[("", use_dpi.upcast_ref()), ("DPI", dpi.upcast_ref())]);
    dialog(ui, "Set DPI", &body, "Apply", move |ui| {
        ui.edit(|e| e.set_dpi(use_dpi.is_active().then(|| dpi.value())));
        true
    });
}

/// Links two spin buttons so changing one updates the other when `lock` is active.
fn lock_aspect(a: &SpinButton, b: &SpinButton, lock: &CheckButton, ratio: f64) {
    let busy = Rc::new(Cell::new(false));
    for (from, to, r) in [
        (a.clone(), b.clone(), 1.0 / ratio),
        (b.clone(), a.clone(), ratio),
    ] {
        let lock = lock.clone();
        let busy = busy.clone();
        from.connect_value_changed(move |from| {
            if busy.get() || !lock.is_active() {
                return;
            }
            busy.set(true);
            let digits = to.digits();
            let value = from.value() * r;
            to.set_value(if digits == 0 {
                value.round().max(1.0)
            } else {
                value
            });
            busy.set(false);
        });
    }
}

/// RES-001, RES-002.
fn resize(ui: &Rc<Ui>, item: ItemId) {
    let Some(i) = ui
        .read(|e| e.project().state().item(item).copied())
        .flatten()
    else {
        return;
    };
    let width = spin(1.0, 100_000.0, 1.0, 0, f64::from(i.width));
    let height = spin(1.0, 100_000.0, 1.0, 0, f64::from(i.height));
    let lock = CheckButton::with_label("Keep aspect ratio");
    lock.set_active(true);
    let (w0, h0) = (i.width, i.height);
    lock_aspect(&width, &height, &lock, f64::from(w0) / f64::from(h0));
    let filter = filter_dropdown(i.filter);
    let body = form(&[
        ("Width (px)", width.upcast_ref()),
        ("Height (px)", height.upcast_ref()),
        ("", lock.upcast_ref()),
        ("Resampling", filter.upcast_ref()),
    ]);
    dialog(ui, "Resize image", &body, "Resize", move |ui| {
        let w = width.value() as u32;
        let h = if lock.is_active() {
            proportional(w0, h0, w)
        } else {
            height.value() as u32
        };
        let filter = Filter::ALL
            .get(filter.selected() as usize)
            .copied()
            .unwrap_or_default();
        ui.edit(|e| e.resize_item(item, w, h, filter));
        true
    });
}

/// RES-004.
fn print_size(ui: &Rc<Ui>, item: ItemId) {
    let Some((i, dpi0)) = ui
        .read(|editor| {
            let state = editor.project().state();
            let i = state.item(item).copied()?;
            Some((i, state.canvas.dpi.unwrap_or(300.0)))
        })
        .flatten()
    else {
        return;
    };
    let unit = dropdown(&["inches", "cm"], 0);
    let width = spin(0.01, 10_000.0, 0.1, 2, f64::from(i.width) / dpi0);
    let height = spin(0.01, 10_000.0, 0.1, 2, f64::from(i.height) / dpi0);
    let dpi = spin(1.0, 10_000.0, 1.0, 0, dpi0);
    let lock = CheckButton::with_label("Keep aspect ratio");
    lock.set_active(true);
    lock_aspect(
        &width,
        &height,
        &lock,
        f64::from(i.width) / f64::from(i.height),
    );
    let filter = filter_dropdown(i.filter);
    let (w_ref, h_ref, lock_ref) = (width.clone(), height.clone(), lock.clone());
    unit.connect_selected_notify(move |unit| {
        let factor = if unit.selected() == 1 {
            2.54
        } else {
            1.0 / 2.54
        };
        // Convert both values directly; the aspect lock would convert the other twice.
        let locked = lock_ref.is_active();
        lock_ref.set_active(false);
        w_ref.set_value(w_ref.value() * factor);
        h_ref.set_value(h_ref.value() * factor);
        lock_ref.set_active(locked);
    });
    let body = form(&[
        ("Unit", unit.upcast_ref()),
        ("Width", width.upcast_ref()),
        ("Height", height.upcast_ref()),
        ("", lock.upcast_ref()),
        ("DPI", dpi.upcast_ref()),
        ("Resampling", filter.upcast_ref()),
    ]);
    dialog(ui, "Print size", &body, "Apply", move |ui| {
        let unit = if unit.selected() == 1 {
            PrintUnit::Centimeters
        } else {
            PrintUnit::Inches
        };
        let filter = Filter::ALL
            .get(filter.selected() as usize)
            .copied()
            .unwrap_or_default();
        ui.edit(|e| {
            e.set_print_size(
                item,
                width.value(),
                height.value(),
                unit,
                dpi.value(),
                filter,
            );
        });
        true
    });
}

/// PLACE-008.
fn rotate(ui: &Rc<Ui>, item: ItemId) {
    let current = ui
        .read(|e| e.project().state().item(item).map(|i| i.rotation))
        .flatten()
        .unwrap_or(0.0);
    let angle = spin(-360.0, 360.0, 0.1, 1, current);
    let body = form(&[("Angle (degrees, clockwise)", angle.upcast_ref())]);
    dialog(ui, "Rotate", &body, "Rotate", move |ui| {
        ui.edit(|e| e.rotate_to(item, angle.value()));
        true
    });
}

/// BLEND-007.
fn feather(ui: &Rc<Ui>, item: ItemId) {
    let Some(f) = ui.read(|e| e.default_feather(item)) else {
        return;
    };
    let edges = [
        CheckButton::with_label("Left"),
        CheckButton::with_label("Right"),
        CheckButton::with_label("Top"),
        CheckButton::with_label("Bottom"),
    ];
    for (check, on) in edges.iter().zip([f.left, f.right, f.top, f.bottom]) {
        check.set_active(on);
    }
    let edge_row = GBox::new(GOrientation::Horizontal, 6);
    for check in &edges {
        edge_row.append(check);
    }
    let width = spin(1.0, 100_000.0, 1.0, 0, f.width);
    let curve = curve_dropdown(f.curve);
    let body = form(&[
        ("Edges", edge_row.upcast_ref()),
        ("Width (px)", width.upcast_ref()),
        ("Curve", curve.upcast_ref()),
    ]);
    dialog(ui, "Feather edges", &body, "Apply", move |ui| {
        let feather = Feather {
            left: edges[0].is_active(),
            right: edges[1].is_active(),
            top: edges[2].is_active(),
            bottom: edges[3].is_active(),
            width: width.value(),
            curve: Curve::ALL
                .get(curve.selected() as usize)
                .copied()
                .unwrap_or_default(),
        };
        ui.edit(|e| e.set_feather(item, feather.any_edge().then_some(feather)));
        true
    });
}

/// BLEND-003, BLEND-008.
fn transition(ui: &Rc<Ui>, id: TransitionId) {
    let Some(t) = ui
        .read(|e| e.project().state().transition(id).copied())
        .flatten()
    else {
        return;
    };
    let remove = Button::with_label("Remove transition");
    remove.add_css_class("destructive-action");
    let window = match t.kind {
        TransitionKind::GradientSeam {
            orientation,
            offset,
            width,
            curve,
        } => {
            let orient = dropdown(
                &["Horizontal", "Vertical"],
                usize::from(orientation == Orientation::Vertical),
            );
            let band = spin(0.0, 100_000.0, 1.0, 0, width);
            let curve_dd = curve_dropdown(curve);
            let body = form(&[
                ("Orientation", orient.upcast_ref()),
                ("Band width (px)", band.upcast_ref()),
                ("Curve", curve_dd.upcast_ref()),
                ("", remove.upcast_ref()),
            ]);
            dialog(ui, "Gradient seam", &body, "Apply", move |ui| {
                let orientation = if orient.selected() == 1 {
                    Orientation::Vertical
                } else {
                    Orientation::Horizontal
                };
                let kind = TransitionKind::GradientSeam {
                    orientation,
                    offset,
                    width: band.value(),
                    curve: Curve::ALL
                        .get(curve_dd.selected() as usize)
                        .copied()
                        .unwrap_or_default(),
                };
                ui.edit(|e| e.edit_transition(id, kind));
                true
            })
        }
        TransitionKind::Crossfade { ratio } => {
            let percent = spin(0.0, 100.0, 1.0, 0, ratio * 100.0);
            let body = form(&[
                ("Upper image opacity (%)", percent.upcast_ref()),
                ("", remove.upcast_ref()),
            ]);
            dialog(ui, "Crossfade", &body, "Apply", move |ui| {
                let kind = TransitionKind::Crossfade {
                    ratio: percent.value() / 100.0,
                };
                ui.edit(|e| e.edit_transition(id, kind));
                true
            })
        }
    };
    let weak = Rc::downgrade(ui);
    remove.connect_clicked(move |_| {
        window.close();
        if let Some(ui) = weak.upgrade() {
            ui.command(Command::RemoveTransition(id));
        }
    });
}

/// META-002 to META-004.
fn exif_viewer(ui: &Rc<Ui>, item: Option<ItemId>) {
    let Some((items, names)) = ui.edit(|e| {
        let items = e.items_with_exif();
        let names: Vec<String> = items.iter().map(|&i| e.item_name(i)).collect();
        (items, names)
    }) else {
        return;
    };
    if items.is_empty() {
        ui.error("No image has EXIF metadata.");
        return;
    }
    let Some(current) = item
        .filter(|i| items.contains(i))
        .or(items.first().copied())
    else {
        return;
    };
    let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let chooser = dropdown(
        &name_refs,
        items.iter().position(|&i| i == current).unwrap_or(0),
    );
    chooser.set_visible(items.len() > 1);
    let table = GBox::new(GOrientation::Vertical, 0);
    let scroller = ScrolledWindow::builder()
        .min_content_width(520)
        .min_content_height(420)
        .child(&table)
        .build();
    let strip = Button::with_label("Strip metadata");
    strip.add_css_class("destructive-action");
    let body = GBox::new(GOrientation::Vertical, 12);
    body.append(&chooser);
    body.append(&scroller);
    body.append(&strip);

    let fill = {
        let ui = ui.clone();
        let table = table.clone();
        move |item: ItemId| {
            while let Some(child) = table.first_child() {
                table.remove(&child);
            }
            let entries = ui.edit(|e| e.exif_entries(item)).unwrap_or_default();
            let grid = Grid::builder().row_spacing(4).column_spacing(16).build();
            let mut row = 0;
            let mut group = None;
            for entry in entries {
                if group != Some(entry.group) {
                    group = Some(entry.group);
                    let header = Label::new(None);
                    header.set_markup(&format!("<b>{}</b>", entry.group.label()));
                    header.set_halign(Align::Start);
                    header.set_margin_top(if row == 0 { 0 } else { 10 });
                    grid.attach(&header, 0, row, 2, 1);
                    row = row.saturating_add(1);
                }
                let name = Label::new(Some(&entry.name));
                name.set_halign(Align::Start);
                name.add_css_class("dim-label");
                let value = Label::new(Some(&entry.value));
                value.set_halign(Align::Start);
                value.set_selectable(true);
                value.set_wrap(true);
                grid.attach(&name, 0, row, 1, 1);
                grid.attach(&value, 1, row, 1, 1);
                row = row.saturating_add(1);
            }
            table.append(&grid);
        }
    };
    fill(current);
    let selected = Rc::new(Cell::new(current));
    let s = selected.clone();
    let items_for_chooser = items.clone();
    let fill_for_chooser = fill.clone();
    chooser.connect_selected_notify(move |c| {
        if let Some(&item) = items_for_chooser.get(c.selected() as usize) {
            s.set(item);
            fill_for_chooser(item);
        }
    });

    let window = Window::builder()
        .title("EXIF metadata")
        .modal(true)
        .transient_for(&ui.window)
        .build();
    let content = GBox::new(GOrientation::Vertical, 12);
    content.set_margin_top(18);
    content.set_margin_bottom(18);
    content.set_margin_start(18);
    content.set_margin_end(18);
    content.append(&body);
    let close = Button::with_label("Close");
    close.set_halign(Align::End);
    content.append(&close);
    window.set_child(Some(&content));
    let w = window.clone();
    close.connect_clicked(move |_| w.close());
    let w = window.clone();
    let weak = Rc::downgrade(ui);
    strip.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            ui.edit(|e| e.strip_metadata(selected.get()));
            ui.refresh();
        }
        w.close();
    });
    window.present();
}

/// PROJ-005: locate a missing source.
fn locate(ui: &Rc<Ui>, source: SourceId) {
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let dialog = FileDialog::builder()
            .title("Locate image")
            .modal(true)
            .filters(&filters(&[image_filter()]))
            .build();
        start_in_last_folder(&ui, &dialog);
        let Ok(file) = dialog.open_future(Some(&ui.window)).await else {
            return;
        };
        if let Some(path) = picked(&ui, &file) {
            ui.edit(|e| e.relink(source, &path));
            ui.refresh();
        }
    });
}

/// EXPORT-001 to EXPORT-004, CONV-002, CONV-003, META-005.
fn export(ui: &Rc<Ui>) {
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let Some((empty, stem)) = ui.read(|e| {
            let stem = e
                .path()
                .and_then(|p| p.file_stem())
                .map(|s| s.to_string_lossy().into_owned());
            (e.is_empty(), stem)
        }) else {
            return;
        };
        if empty {
            ui.error("There is nothing to export.");
            return;
        }
        let stem = stem.unwrap_or_else(|| "export".to_owned());
        let dialog = FileDialog::builder()
            .title("Export")
            .modal(true)
            .initial_name(format!("{stem}.png"))
            .build();
        start_in_last_folder(&ui, &dialog);
        let Ok(file) = dialog.save_future(Some(&ui.window)).await else {
            return;
        };
        let Some(path) = picked(&ui, &file) else {
            return;
        };
        let Some(overwrites) = ui.read(|e| e.export_overwrites_source(&path)) else {
            return;
        };
        if overwrites {
            let choice = gtk4::AlertDialog::builder()
                .message("Overwrite a source image?")
                .detail(format!(
                    "{} is used by this project. Exporting will replace it.",
                    path.display()
                ))
                .buttons(["Cancel", "Overwrite"])
                .cancel_button(0)
                .default_button(0)
                .modal(true)
                .build()
                .choose_future(Some(&ui.window))
                .await;
            if choice != Ok(1) {
                return;
            }
        }
        export_options(&ui, path);
    });
}

fn export_options(ui: &Rc<Ui>, path: PathBuf) {
    let Some((defaults, items, item_names)) = ui.edit(|e| {
        let items = e.items_with_exif();
        let names: Vec<String> = items.iter().map(|&i| e.item_name(i)).collect();
        (e.export_defaults(&path), items, names)
    }) else {
        return;
    };
    let formats = Format::WRITABLE;
    let labels: Vec<&str> = formats.iter().map(|f| f.label()).collect();
    let format = dropdown(
        &labels,
        formats
            .iter()
            .position(|f| *f == defaults.format)
            .unwrap_or(0),
    );
    let quality = spin(1.0, 100.0, 1.0, 0, f64::from(defaults.quality));
    quality.set_sensitive(defaults.format.is_lossy());
    let q = quality.clone();
    format.connect_selected_notify(move |d| {
        if let Some(f) = formats.get(d.selected() as usize) {
            q.set_sensitive(f.is_lossy());
        }
    });

    let mut names = vec!["None".to_owned()];
    names.extend(item_names);
    let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let source_index = defaults
        .metadata
        .and_then(|m| items.iter().position(|&i| i == m))
        .map(|i| i.saturating_add(1))
        .unwrap_or(0);
    let source = dropdown(&name_refs, source_index);
    let groups_box = GBox::new(GOrientation::Vertical, 2);
    let checks: Vec<(ExifGroup, CheckButton)> = ExifGroup::ALL
        .into_iter()
        .map(|g| {
            let c = CheckButton::with_label(g.label());
            c.set_active(defaults.groups.contains(&g));
            groups_box.append(&c);
            (g, c)
        })
        .collect();
    groups_box.set_sensitive(source_index > 0);
    let gb = groups_box.clone();
    source.connect_selected_notify(move |d| gb.set_sensitive(d.selected() > 0));

    let body = form(&[
        ("Format", format.upcast_ref()),
        ("Quality", quality.upcast_ref()),
        ("EXIF metadata from", source.upcast_ref()),
        ("Include", groups_box.upcast_ref()),
    ]);
    dialog(ui, "Export", &body, "Export", move |ui| {
        let mut settings = defaults.clone();
        if let Some(&f) = formats.get(format.selected() as usize) {
            settings.format = f;
        }
        if Format::from_path(&settings.path) != Some(settings.format)
            && let Some(ext) = settings.format.extensions().first()
        {
            settings.path.set_extension(ext);
        }
        // A changed extension is a file the user has not confirmed (EXPORT-003).
        if settings.path != defaults.path {
            // keep the dialog open if the editor is busy
            let Some(overwrites) = ui.read(|e| e.export_overwrites_source(&settings.path)) else {
                return false;
            };
            if overwrites || settings.path.exists() {
                ui.error(&format!(
                    "{} already exists. Export again and choose that name to replace it.",
                    settings.path.display()
                ));
                return false;
            }
        }
        settings.quality = quality.value() as u8;
        settings.metadata = match source.selected() {
            0 => None,
            n => items.get((n as usize).saturating_sub(1)).copied(),
        };
        settings.groups = checks
            .iter()
            .filter(|(_, c)| c.is_active())
            .map(|(g, _)| *g)
            .collect();
        match ui.edit(|e| e.export(&settings)) {
            Some(Ok(())) => {}
            Some(Err(e)) => {
                ui.error(&e);
                return false;
            }
            None => return false,
        }
        true
    });
}
