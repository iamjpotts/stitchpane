//! GTK 4 user interface. All editing behavior lives in [`stitchpane_core::editor::Editor`];
//! this crate shows its frame, forwards input to it, and provides the dialogs.

mod dialogs;

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::process::ExitCode;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GBox, Button, DrawingArea, DropDown,
    EventControllerKey, EventControllerMotion, EventControllerScroll, EventControllerScrollFlags,
    GestureClick, GestureDrag, HeaderBar, Label, MenuButton, Orientation as GOrientation, Overlay,
    PopoverMenu, SpinButton, cairo, gdk, gio, glib,
};
use stitchpane_core::editor::{
    AspectChoice, Button as PointerButton, Command, Cursor, Editor, Key, KeyOutcome, Modifiers,
};
use stitchpane_core::geometry::{PixelRect, Point};
use stitchpane_core::model::{Light, StrokeKind, ToneRange};
use stitchpane_core::{APP_ID, APP_NAME};

pub fn run() -> ExitCode {
    let app = Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_OPEN | gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.connect_activate(|app| {
        Ui::new(app, Vec::new());
    });
    app.connect_open(|app, files, _| {
        let paths = files.iter().filter_map(|f| f.path()).collect();
        Ui::new(app, paths);
    });
    let code = app.run();
    ExitCode::from(code.get())
}

/// Crop tool controls (CROP-002, CROP-003, CROP-004).
struct CropBar {
    container: GBox,
    left: SpinButton,
    top: SpinButton,
    width: SpinButton,
    height: SpinButton,
    aspect: DropDown,
    custom_w: SpinButton,
    custom_h: SpinButton,
}

/// Smear tool controls (SMEAR-001 to SMEAR-003).
struct SmearBar {
    container: GBox,
    kind: DropDown,
    size: SpinButton,
    strength: SpinButton,
}

/// Light tool controls (LIGHT-001 to LIGHT-004).
struct LightBar {
    container: GBox,
    exposure: SpinButton,
    shadows: SpinButton,
    highlights: SpinButton,
    radius: SpinButton,
    black: SpinButton,
    midtones: SpinButton,
    white: SpinButton,
}

/// Dodge and burn tool controls (DODGE-001 to DODGE-003).
struct DodgeBar {
    container: GBox,
    kind: DropDown,
    range: DropDown,
    size: SpinButton,
    strength: SpinButton,
}

pub(crate) struct Ui {
    editor: RefCell<Editor>,
    window: ApplicationWindow,
    canvas: DrawingArea,
    save: Button,
    undo: Button,
    redo: Button,
    menu: MenuButton,
    status: Label,
    exif: Button,
    missing: GBox,
    missing_label: Label,
    hint: Label,
    crop: CropBar,
    smear: SmearBar,
    light: LightBar,
    dodge: DodgeBar,
    zoom: MenuButton,
    /// Last pointer position over the canvas.
    pointer: Cell<Point>,
    drag_start: Cell<Point>,
    /// Suppresses control callbacks while refresh() updates them.
    updating: Cell<bool>,
    /// Whether a timer is waiting for background resamples (UI-009).
    polling: Cell<bool>,
}

fn modifiers(state: gdk::ModifierType) -> Modifiers {
    Modifiers {
        shift: state.contains(gdk::ModifierType::SHIFT_MASK),
        ctrl: state.contains(gdk::ModifierType::CONTROL_MASK),
        alt: state.contains(gdk::ModifierType::ALT_MASK),
    }
}

fn map_key(key: gdk::Key) -> Option<Key> {
    let name = key.name()?;
    Some(match name.as_str() {
        "Left" | "KP_Left" => Key::Left,
        "Right" | "KP_Right" => Key::Right,
        "Up" | "KP_Up" => Key::Up,
        "Down" | "KP_Down" => Key::Down,
        "bracketleft" | "braceleft" => Key::BracketLeft,
        "bracketright" | "braceright" => Key::BracketRight,
        "Escape" => Key::Escape,
        "Return" | "KP_Enter" => Key::Enter,
        "Delete" | "KP_Delete" => Key::Delete,
        "BackSpace" => Key::Backspace,
        "plus" | "KP_Add" | "equal" => Key::Plus,
        "minus" | "KP_Subtract" | "underscore" => Key::Minus,
        _ => Key::Char(key.to_lower().to_unicode()?),
    })
}

fn cursor_name(cursor: Cursor) -> &'static str {
    match cursor {
        Cursor::Default => "default",
        Cursor::Move => "move",
        Cursor::Grab => "grab",
        Cursor::ResizeHorizontal => "ew-resize",
        Cursor::ResizeVertical => "ns-resize",
        Cursor::ResizeDiagonal => "nwse-resize",
        Cursor::Rotate => "crosshair",
        Cursor::Brush => "crosshair",
    }
}

/// The window action for a zoom command (UI-010).
fn zoom_action(command: &Command) -> Option<&'static str> {
    Some(match command {
        Command::ZoomIn => "win.zoom-in",
        Command::ZoomOut => "win.zoom-out",
        Command::ZoomFit => "win.zoom-fit",
        Command::Zoom100 => "win.zoom-100",
        _ => return None,
    })
}

fn spin(max: f64) -> SpinButton {
    let s = SpinButton::with_range(0.0, max, 1.0);
    s.set_width_chars(6);
    s
}

/// A spin button from `min` to `max` in steps of `step`, showing `digits` decimals.
fn spin_range(min: f64, max: f64, step: f64, digits: u32) -> SpinButton {
    let s = SpinButton::with_range(min, max, step);
    s.set_digits(digits);
    s.set_width_chars(5);
    s
}

/// A tool bar row with the margins the crop and smear bars use.
fn tool_bar() -> GBox {
    let bar = GBox::new(GOrientation::Horizontal, 6);
    bar.set_margin_start(6);
    bar.set_margin_end(6);
    bar.set_margin_top(6);
    bar.set_margin_bottom(6);
    bar
}

/// A horizontal space that takes up the rest of a tool bar.
fn bar_spacer() -> GBox {
    let spacer = GBox::new(GOrientation::Horizontal, 0);
    spacer.set_hexpand(true);
    spacer
}

impl Ui {
    fn new(app: &Application, paths: Vec<PathBuf>) -> Rc<Ui> {
        let window = ApplicationWindow::builder()
            .application(app)
            .title(APP_NAME)
            .default_width(1200)
            .default_height(800)
            .build();

        let header = HeaderBar::new();
        let open = Button::from_icon_name("document-open-symbolic");
        open.set_tooltip_text(Some("Open (Ctrl+O)"));
        let add = Button::from_icon_name("list-add-symbolic");
        add.set_tooltip_text(Some("Add images"));
        let save = Button::from_icon_name("document-save-symbolic");
        save.set_tooltip_text(Some("Save (Ctrl+S)"));
        let undo = Button::from_icon_name("edit-undo-symbolic");
        let redo = Button::from_icon_name("edit-redo-symbolic");
        header.pack_start(&open);
        header.pack_start(&add);
        header.pack_start(&save);
        header.pack_start(&undo);
        header.pack_start(&redo);
        let menu = MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .tooltip_text("Main menu")
            .build();
        header.pack_end(&menu);
        window.set_titlebar(Some(&header));

        let canvas = DrawingArea::builder()
            .hexpand(true)
            .vexpand(true)
            .focusable(true)
            .can_focus(true)
            .build();
        let hint = Label::new(Some("Open an image or drop image files here"));
        hint.add_css_class("title-2");
        hint.set_can_target(false);
        let overlay = Overlay::new();
        overlay.set_child(Some(&canvas));
        overlay.add_overlay(&hint);

        let crop = CropBar {
            container: GBox::new(GOrientation::Horizontal, 6),
            left: spin(1e6),
            top: spin(1e6),
            width: spin(1e6),
            height: spin(1e6),
            aspect: DropDown::from_strings(&[
                "Free", "Original", "1:1", "4:3", "3:2", "16:9", "Custom",
            ]),
            custom_w: spin(1000.0),
            custom_h: spin(1000.0),
        };
        crop.custom_w.set_range(1.0, 1000.0);
        crop.custom_h.set_range(1.0, 1000.0);
        crop.custom_w.set_value(3.0);
        crop.custom_h.set_value(4.0);
        crop.container.set_margin_start(6);
        crop.container.set_margin_end(6);
        crop.container.set_margin_top(6);
        crop.container.set_margin_bottom(6);
        for (label, widget) in [
            ("Left", &crop.left),
            ("Top", &crop.top),
            ("Width", &crop.width),
            ("Height", &crop.height),
        ] {
            crop.container.append(&Label::new(Some(label)));
            crop.container.append(widget);
        }
        crop.container.append(&Label::new(Some("Aspect")));
        crop.container.append(&crop.aspect);
        crop.container.append(&crop.custom_w);
        crop.container.append(&Label::new(Some(":")));
        crop.container.append(&crop.custom_h);
        let apply_crop = Button::with_label("Apply");
        apply_crop.add_css_class("suggested-action");
        let cancel_crop = Button::with_label("Cancel");
        let spacer = GBox::new(GOrientation::Horizontal, 0);
        spacer.set_hexpand(true);
        crop.container.append(&spacer);
        crop.container.append(&cancel_crop);
        crop.container.append(&apply_crop);

        let kinds: Vec<&str> = StrokeKind::SMEAR.iter().map(|k| k.label()).collect();
        let smear = SmearBar {
            container: GBox::new(GOrientation::Horizontal, 6),
            kind: DropDown::from_strings(&kinds),
            size: spin(5000.0),
            strength: spin(100.0),
        };
        smear.size.set_range(1.0, 5000.0);
        smear.strength.set_range(1.0, 100.0);
        for margin in [
            GBox::set_margin_start,
            GBox::set_margin_end,
            GBox::set_margin_top,
            GBox::set_margin_bottom,
        ] {
            margin(&smear.container, 6);
        }
        smear.container.append(&Label::new(Some("Smear")));
        smear.container.append(&smear.kind);
        smear.container.append(&Label::new(Some("Size")));
        smear.container.append(&smear.size);
        smear.container.append(&Label::new(Some("Strength %")));
        smear.container.append(&smear.strength);
        let smear_spacer = GBox::new(GOrientation::Horizontal, 0);
        smear_spacer.set_hexpand(true);
        smear.container.append(&smear_spacer);
        let smear_done = Button::with_label("Done");
        smear_done.add_css_class("suggested-action");
        smear.container.append(&smear_done);

        // Two rows so the bar fits the window: exposure and shadows and highlights, then
        // levels and the buttons.
        let light = LightBar {
            container: GBox::new(GOrientation::Vertical, 0),
            exposure: spin_range(-5.0, 5.0, 0.1, 1),
            shadows: spin_range(0.0, 100.0, 1.0, 0),
            highlights: spin_range(0.0, 100.0, 1.0, 0),
            radius: spin_range(1.0, 100_000.0, 1.0, 0),
            black: spin_range(0.0, 254.0, 1.0, 0),
            midtones: spin_range(0.1, 9.99, 0.01, 2),
            white: spin_range(1.0, 255.0, 1.0, 0),
        };
        let light_rows = [tool_bar(), tool_bar()];
        let [first, second] = &light_rows;
        first.set_margin_bottom(0);
        first.append(&Label::new(Some("Light")));
        second.append(&Label::new(Some("Levels")));
        for (row, label, widget) in [
            (first, "Exposure", &light.exposure),
            (first, "Shadows %", &light.shadows),
            (first, "Highlights %", &light.highlights),
            (first, "Radius", &light.radius),
            (second, "Black", &light.black),
            (second, "Midtones", &light.midtones),
            (second, "White", &light.white),
        ] {
            row.append(&Label::new(Some(label)));
            row.append(widget);
        }
        second.append(&bar_spacer());
        let reset_light = Button::with_label("Reset");
        let cancel_light = Button::with_label("Cancel");
        let apply_light = Button::with_label("Apply");
        apply_light.add_css_class("suggested-action");
        second.append(&reset_light);
        second.append(&cancel_light);
        second.append(&apply_light);
        light.container.append(first);
        light.container.append(second);

        let kinds: Vec<&str> = StrokeKind::DODGE_BURN.iter().map(|k| k.label()).collect();
        let ranges: Vec<&str> = ToneRange::ALL.iter().map(|r| r.label()).collect();
        let dodge = DodgeBar {
            container: tool_bar(),
            kind: DropDown::from_strings(&kinds),
            range: DropDown::from_strings(&ranges),
            size: spin(5000.0),
            strength: spin(100.0),
        };
        dodge.size.set_range(1.0, 5000.0);
        dodge.strength.set_range(1.0, 100.0);
        dodge.container.append(&Label::new(Some("Dodge and burn")));
        dodge.container.append(&dodge.kind);
        dodge.container.append(&Label::new(Some("Range")));
        dodge.container.append(&dodge.range);
        dodge.container.append(&Label::new(Some("Size")));
        dodge.container.append(&dodge.size);
        dodge.container.append(&Label::new(Some("Strength %")));
        dodge.container.append(&dodge.strength);
        dodge.container.append(&bar_spacer());
        let dodge_done = Button::with_label("Done");
        dodge_done.add_css_class("suggested-action");
        dodge.container.append(&dodge_done);

        let status_bar = GBox::new(GOrientation::Horizontal, 12);
        status_bar.set_margin_start(8);
        status_bar.set_margin_end(8);
        status_bar.set_margin_top(2);
        status_bar.set_margin_bottom(2);
        let status = Label::new(None);
        status.set_halign(Align::Start);
        // UI-010: the zoom level opens the View menu's zoom choices.
        let zoom_menu = gio::Menu::new();
        for entry in Editor::zoom_menu() {
            let action = entry.command.as_ref().and_then(zoom_action);
            zoom_menu.append(Some(&entry.label), action);
        }
        let zoom = MenuButton::builder()
            .menu_model(&zoom_menu)
            .direction(gtk4::ArrowType::Up)
            .tooltip_text("Zoom")
            .build();
        zoom.add_css_class("flat");
        let status_spacer = GBox::new(GOrientation::Horizontal, 0);
        status_spacer.set_hexpand(true);
        let missing = GBox::new(GOrientation::Horizontal, 6);
        let missing_label = Label::new(None);
        let locate = Button::with_label("Locate…");
        locate.add_css_class("flat");
        missing.append(&missing_label);
        missing.append(&locate);
        let exif = Button::new();
        exif.add_css_class("flat");
        exif.set_tooltip_text(Some("Show EXIF metadata"));
        status_bar.append(&status);
        status_bar.append(&zoom);
        status_bar.append(&status_spacer);
        status_bar.append(&missing);
        status_bar.append(&exif);

        let layout = GBox::new(GOrientation::Vertical, 0);
        layout.append(&crop.container);
        layout.append(&smear.container);
        layout.append(&light.container);
        layout.append(&dodge.container);
        layout.append(&overlay);
        layout.append(&status_bar);
        window.set_child(Some(&layout));

        let mut editor = Editor::new();
        editor.set_background_rendering(true);
        let ui = Rc::new(Ui {
            editor: RefCell::new(editor),
            window: window.clone(),
            canvas: canvas.clone(),
            save: save.clone(),
            undo: undo.clone(),
            redo: redo.clone(),
            menu,
            status,
            exif: exif.clone(),
            missing,
            missing_label,
            hint,
            crop,
            smear,
            light,
            dodge,
            zoom,
            pointer: Cell::new(Point::default()),
            drag_start: Cell::new(Point::default()),
            updating: Cell::new(false),
            polling: Cell::new(false),
        });

        // Callbacks hold weak references; the window owns the UI until it is destroyed.
        let keep = RefCell::new(Some(ui.clone()));
        window.connect_destroy(move |_| {
            if let Ok(mut keep) = keep.try_borrow_mut() {
                keep.take();
            }
        });

        ui.install_actions();
        ui.connect_canvas();
        ui.connect_crop_bar(&apply_crop, &cancel_crop);
        ui.connect_smear_bar(&smear_done);
        ui.connect_light_bar(&reset_light, &cancel_light, &apply_light);
        ui.connect_dodge_bar(&dodge_done);

        let weak = Rc::downgrade(&ui);
        let on = move |command: Command| {
            let weak = weak.clone();
            move |_: &Button| {
                if let Some(ui) = weak.upgrade() {
                    ui.command(command.clone());
                }
            }
        };
        open.connect_clicked(on(Command::Open));
        add.connect_clicked(on(Command::AddImages));
        save.connect_clicked(on(Command::Save));
        undo.connect_clicked(on(Command::Undo));
        redo.connect_clicked(on(Command::Redo));
        let weak = Rc::downgrade(&ui);
        exif.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade()
                && let Some(item) = ui.edit(|e| e.exif_viewer_item())
            {
                ui.command(Command::ExifViewer(item));
            }
        });
        let weak = Rc::downgrade(&ui);
        locate.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                let first = ui
                    .edit(|e| e.missing_sources().first().map(|m| m.0))
                    .flatten();
                if let Some(source) = first {
                    ui.command(Command::LocateSource(source));
                }
            }
        });

        // UI-002: offer to save before closing.
        let weak = Rc::downgrade(&ui);
        window.connect_close_request(move |window| {
            let Some(ui) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            // if the editor is busy, ask rather than risk losing work
            if !ui.read(|e| e.needs_save_prompt()).unwrap_or(true) {
                return glib::Propagation::Proceed;
            }
            let window = window.clone();
            glib::spawn_future_local(async move {
                if dialogs::confirm_discard(&ui).await {
                    ui.edit(|e| *e = Editor::new());
                    window.destroy();
                }
            });
            glib::Propagation::Stop
        });

        window.present();
        if !paths.is_empty() {
            let result = if let [path] = paths.as_slice() {
                ui.edit(|e| e.open(path))
            } else {
                ui.edit(|e| e.open_images(&paths))
            };
            if let Some(Err(e)) = result {
                ui.error(&e);
            }
        }
        ui.refresh();
        canvas.grab_focus();
        ui
    }

    fn install_actions(self: &Rc<Self>) {
        let actions: [(&str, Command); 14] = [
            ("open", Command::Open),
            ("add", Command::AddImages),
            ("save", Command::Save),
            ("save-as", Command::SaveAs),
            ("export", Command::Export),
            ("undo", Command::Undo),
            ("redo", Command::Redo),
            ("zoom-in", Command::ZoomIn),
            ("zoom-out", Command::ZoomOut),
            ("zoom-fit", Command::ZoomFit),
            ("zoom-100", Command::Zoom100),
            ("canvas-size", Command::CanvasSize),
            ("dpi", Command::SetDpi),
            ("exif", Command::ExifViewer(None)),
        ];
        for (name, command) in actions {
            let action = gio::SimpleAction::new(name, None);
            let weak = Rc::downgrade(self);
            action.connect_activate(move |_, _| {
                if let Some(ui) = weak.upgrade() {
                    let command = match &command {
                        Command::ExifViewer(None) => {
                            let Some(item) = ui.edit(|e| e.exif_viewer_item()) else {
                                return;
                            };
                            Command::ExifViewer(item)
                        }
                        other => other.clone(),
                    };
                    ui.command(command);
                }
            });
            self.window.add_action(&action);
        }
        let snapping = gio::SimpleAction::new_stateful("snapping", None, &true.to_variant());
        let weak = Rc::downgrade(self);
        snapping.connect_activate(move |action, _| {
            if let Some(ui) = weak.upgrade() {
                ui.command(Command::ToggleSnapping);
                if let Some(snapping) = ui.read(|e| e.snapping()) {
                    action.set_state(&snapping.to_variant());
                }
            }
        });
        self.window.add_action(&snapping);
    }

    /// The main menu, rebuilt on refresh so undo and redo show their step (UNDO-003).
    fn menu_model(&self) -> gio::Menu {
        let (undo, redo) = self
            .read(|e| (e.undo_label(), e.redo_label()))
            .unwrap_or_default();
        let item = |label: &str, action: &str, accel: Option<&str>| {
            let item = gio::MenuItem::new(Some(label), Some(action));
            if let Some(accel) = accel {
                item.set_attribute_value("accel", Some(&accel.to_variant()));
            }
            item
        };
        let menu = gio::Menu::new();
        let file = gio::Menu::new();
        file.append_item(&item("Open…", "win.open", Some("<Control>o")));
        file.append_item(&item("Add images…", "win.add", None));
        file.append_item(&item("Save", "win.save", Some("<Control>s")));
        file.append_item(&item("Save as…", "win.save-as", Some("<Shift><Control>s")));
        file.append_item(&item("Export…", "win.export", Some("<Control>e")));
        menu.append_section(None, &file);
        let edit = gio::Menu::new();
        edit.append_item(&item(
            &undo.unwrap_or_else(|| "Undo".to_owned()),
            "win.undo",
            Some("<Control>z"),
        ));
        edit.append_item(&item(
            &redo.unwrap_or_else(|| "Redo".to_owned()),
            "win.redo",
            Some("<Shift><Control>z"),
        ));
        menu.append_submenu(Some("Edit"), &edit);
        let view = gio::Menu::new();
        view.append_item(&item("Zoom in", "win.zoom-in", Some("<Control>plus")));
        view.append_item(&item("Zoom out", "win.zoom-out", Some("<Control>minus")));
        view.append_item(&item("Fit", "win.zoom-fit", Some("<Control>0")));
        view.append_item(&item("100%", "win.zoom-100", Some("<Control>1")));
        view.append_item(&item("Snapping", "win.snapping", None));
        menu.append_submenu(Some("View"), &view);
        let image = gio::Menu::new();
        image.append_item(&item("Canvas size…", "win.canvas-size", None));
        image.append_item(&item("Set DPI…", "win.dpi", None));
        image.append_item(&item("Show EXIF metadata…", "win.exif", None));
        menu.append_submenu(Some("Image"), &image);
        menu
    }

    fn connect_canvas(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.canvas.connect_resize(move |_, w, h| {
            if let Some(ui) = weak.upgrade() {
                ui.edit(|e| e.set_viewport(f64::from(w), f64::from(h)));
                ui.refresh();
            }
        });
        let weak = Rc::downgrade(self);
        self.canvas.set_draw_func(move |_, cr, w, h| {
            if let Some(ui) = weak.upgrade() {
                ui.draw(cr, w, h);
            }
        });

        let drag = GestureDrag::new();
        drag.set_button(gdk::BUTTON_PRIMARY);
        let weak = Rc::downgrade(self);
        drag.connect_drag_begin(move |gesture, x, y| {
            let Some(ui) = weak.upgrade() else { return };
            ui.canvas.grab_focus();
            let p = Point::new(x, y);
            ui.drag_start.set(p);
            let mods = modifiers(gesture.current_event_state());
            ui.edit(|e| e.pointer_down(p, PointerButton::Primary, mods));
            ui.refresh();
        });
        let weak = Rc::downgrade(self);
        drag.connect_drag_update(move |gesture, dx, dy| {
            let Some(ui) = weak.upgrade() else { return };
            let s = ui.drag_start.get();
            let mods = modifiers(gesture.current_event_state());
            ui.edit(|e| e.pointer_move(Point::new(s.x + dx, s.y + dy), mods));
            ui.update_cursor();
            ui.canvas.queue_draw();
            ui.refresh_crop_bar();
        });
        let weak = Rc::downgrade(self);
        drag.connect_drag_end(move |gesture, dx, dy| {
            let Some(ui) = weak.upgrade() else { return };
            let s = ui.drag_start.get();
            let mods = modifiers(gesture.current_event_state());
            ui.edit(|e| e.pointer_up(Point::new(s.x + dx, s.y + dy), mods));
            ui.refresh();
        });
        self.canvas.add_controller(drag);

        // UI-004: context menus.
        let click = GestureClick::new();
        click.set_button(gdk::BUTTON_SECONDARY);
        let weak = Rc::downgrade(self);
        click.connect_pressed(move |gesture, _, x, y| {
            let Some(ui) = weak.upgrade() else { return };
            let mods = modifiers(gesture.current_event_state());
            let menu = ui
                .edit(|e| e.pointer_down(Point::new(x, y), PointerButton::Secondary, mods))
                .flatten();
            ui.refresh();
            if let Some(menu) = menu {
                dialogs::context_menu(&ui, menu, x, y);
            }
        });
        self.canvas.add_controller(click);

        let motion = EventControllerMotion::new();
        let weak = Rc::downgrade(self);
        motion.connect_motion(move |_, x, y| {
            if let Some(ui) = weak.upgrade() {
                ui.pointer.set(Point::new(x, y));
                ui.update_cursor();
                // SMEAR-001: the brush outline follows the pointer.
                if ui
                    .edit(|e| e.pointer_hover(Point::new(x, y)))
                    .unwrap_or(false)
                {
                    ui.canvas.queue_draw();
                }
            }
        });
        self.canvas.add_controller(motion);

        let scroll = EventControllerScroll::new(EventControllerScrollFlags::BOTH_AXES);
        let weak = Rc::downgrade(self);
        scroll.connect_scroll(move |controller, dx, dy| {
            let Some(ui) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            let mods = modifiers(controller.current_event_state());
            let p = ui.pointer.get();
            ui.edit(|e| e.scroll(p, dx, dy, mods));
            ui.refresh();
            glib::Propagation::Stop
        });
        self.canvas.add_controller(scroll);

        // PLACE-001: drop image files onto the canvas.
        let drop = gtk4::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
        let weak = Rc::downgrade(self);
        drop.connect_drop(move |_, value, x, y| {
            let Some(ui) = weak.upgrade() else {
                return false;
            };
            let Ok(files) = value.get::<gdk::FileList>() else {
                return false;
            };
            let paths: Vec<PathBuf> = files.files().iter().filter_map(|f| f.path()).collect();
            let result = ui.edit(|e| e.add_images(&paths, Some(Point::new(x, y))));
            if let Some(Err(e)) = result {
                ui.error(&e);
            }
            ui.refresh();
            true
        });
        self.canvas.add_controller(drop);

        let keys = EventControllerKey::new();
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, key, _, state| {
            let Some(ui) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            let Some(key) = map_key(key) else {
                return glib::Propagation::Proceed;
            };
            let Some(outcome) = ui.edit(|e| e.key_press(key, modifiers(state))) else {
                return glib::Propagation::Proceed;
            };
            match outcome {
                KeyOutcome::Unhandled => glib::Propagation::Proceed,
                KeyOutcome::Handled => {
                    ui.refresh();
                    glib::Propagation::Stop
                }
                KeyOutcome::Command(command) => {
                    ui.command(command);
                    glib::Propagation::Stop
                }
            }
        });
        self.window.add_controller(keys);
    }

    fn connect_smear_bar(self: &Rc<Self>, done: &Button) {
        let weak = Rc::downgrade(self);
        let apply = move || {
            let Some(ui) = weak.upgrade() else { return };
            if ui.updating.get() {
                return;
            }
            let s = &ui.smear;
            ui.edit(|editor| {
                if let Some(kind) = StrokeKind::SMEAR.get(s.kind.selected() as usize) {
                    editor.set_smear_kind(*kind);
                }
                editor.set_smear_size(s.size.value());
                editor.set_smear_strength(s.strength.value() / 100.0);
            });
            ui.canvas.queue_draw();
        };
        let f = apply.clone();
        self.smear.kind.connect_selected_notify(move |_| f());
        let f = apply.clone();
        self.smear.size.connect_value_changed(move |_| f());
        self.smear.strength.connect_value_changed(move |_| apply());
        let weak = Rc::downgrade(self);
        done.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.command(Command::EndSmear);
            }
        });
    }

    fn refresh_smear_bar(&self) {
        let Some(tool) = self.read(|e| e.smear_tool()) else {
            return;
        };
        self.smear.container.set_visible(tool.is_some());
        let Some(tool) = tool else { return };
        self.updating.set(true);
        let index = StrokeKind::SMEAR.iter().position(|k| *k == tool.kind);
        self.smear.kind.set_selected(index.unwrap_or(0) as u32);
        self.smear.size.set_value(tool.size);
        self.smear
            .strength
            .set_value((tool.strength * 100.0).round());
        self.updating.set(false);
    }

    fn connect_light_bar(self: &Rc<Self>, reset: &Button, cancel: &Button, apply: &Button) {
        let b = &self.light;
        for spin in [
            &b.exposure,
            &b.shadows,
            &b.highlights,
            &b.radius,
            &b.black,
            &b.midtones,
            &b.white,
        ] {
            let weak = Rc::downgrade(self);
            spin.connect_value_changed(move |_| {
                let Some(ui) = weak.upgrade() else { return };
                if ui.updating.get() {
                    return;
                }
                let b = &ui.light;
                let light = Light {
                    exposure: b.exposure.value(),
                    shadows: b.shadows.value() / 100.0,
                    highlights: b.highlights.value() / 100.0,
                    radius: b.radius.value(),
                    black: b.black.value(),
                    white: b.white.value(),
                    midtones: b.midtones.value(),
                };
                ui.edit(|e| e.set_light(light));
                ui.refresh();
            });
        }
        for (button, command) in [
            (reset, Command::ResetLight),
            (cancel, Command::CancelLight),
            (apply, Command::ApplyLight),
        ] {
            let weak = Rc::downgrade(self);
            button.connect_clicked(move |_| {
                if let Some(ui) = weak.upgrade() {
                    ui.command(command.clone());
                }
            });
        }
    }

    fn refresh_light_bar(&self) {
        let Some(tool) = self.read(|e| e.light_tool()) else {
            return;
        };
        self.light.container.set_visible(tool.is_some());
        let Some(tool) = tool else { return };
        let (b, l) = (&self.light, tool.light);
        self.updating.set(true);
        // the white point stays above the black point (LIGHT-004)
        b.black.set_range(0.0, (l.white - 1.0).max(0.0));
        b.white.set_range((l.black + 1.0).min(255.0), 255.0);
        b.exposure.set_value(l.exposure);
        b.shadows.set_value((l.shadows * 100.0).round());
        b.highlights.set_value((l.highlights * 100.0).round());
        b.radius.set_value(l.radius);
        b.black.set_value(l.black);
        b.midtones.set_value(l.midtones);
        b.white.set_value(l.white);
        self.updating.set(false);
    }

    fn connect_dodge_bar(self: &Rc<Self>, done: &Button) {
        let weak = Rc::downgrade(self);
        let apply = move || {
            let Some(ui) = weak.upgrade() else { return };
            if ui.updating.get() {
                return;
            }
            let d = &ui.dodge;
            ui.edit(|editor| {
                if let Some(kind) = StrokeKind::DODGE_BURN.get(d.kind.selected() as usize) {
                    editor.set_dodge_kind(*kind);
                }
                if let Some(range) = ToneRange::ALL.get(d.range.selected() as usize) {
                    editor.set_dodge_range(*range);
                }
                editor.set_dodge_size(d.size.value());
                editor.set_dodge_strength(d.strength.value() / 100.0);
            });
            ui.canvas.queue_draw();
        };
        let f = apply.clone();
        self.dodge.kind.connect_selected_notify(move |_| f());
        let f = apply.clone();
        self.dodge.range.connect_selected_notify(move |_| f());
        let f = apply.clone();
        self.dodge.size.connect_value_changed(move |_| f());
        self.dodge.strength.connect_value_changed(move |_| apply());
        let weak = Rc::downgrade(self);
        done.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.command(Command::EndDodgeBurn);
            }
        });
    }

    fn refresh_dodge_bar(&self) {
        let Some(tool) = self.read(|e| e.dodge_tool()) else {
            return;
        };
        self.dodge.container.set_visible(tool.is_some());
        let Some(tool) = tool else { return };
        self.updating.set(true);
        let kind = StrokeKind::DODGE_BURN.iter().position(|k| *k == tool.kind);
        self.dodge.kind.set_selected(kind.unwrap_or(0) as u32);
        let range = ToneRange::ALL.iter().position(|r| *r == tool.range);
        self.dodge.range.set_selected(range.unwrap_or(0) as u32);
        self.dodge.size.set_value(tool.size);
        self.dodge
            .strength
            .set_value((tool.strength * 100.0).round());
        self.updating.set(false);
    }

    fn connect_crop_bar(self: &Rc<Self>, apply: &Button, cancel: &Button) {
        for spin in [
            &self.crop.left,
            &self.crop.top,
            &self.crop.width,
            &self.crop.height,
        ] {
            let weak = Rc::downgrade(self);
            spin.connect_value_changed(move |_| {
                let Some(ui) = weak.upgrade() else { return };
                if ui.updating.get() {
                    return;
                }
                let c = &ui.crop;
                let rect = PixelRect::new(
                    c.left.value() as u32,
                    c.top.value() as u32,
                    c.width.value().max(1.0) as u32,
                    c.height.value().max(1.0) as u32,
                );
                ui.edit(|e| e.set_crop_rect(rect));
                ui.refresh();
            });
        }
        let weak = Rc::downgrade(self);
        let set_aspect = move || {
            let Some(ui) = weak.upgrade() else { return };
            if ui.updating.get() {
                return;
            }
            let c = &ui.crop;
            let index = c.aspect.selected() as usize;
            let aspect = AspectChoice::PRESETS
                .get(index)
                .copied()
                .unwrap_or_else(|| {
                    AspectChoice::Custom(c.custom_w.value() as u32, c.custom_h.value() as u32)
                });
            ui.edit(|e| e.set_crop_aspect(aspect));
            ui.refresh();
        };
        let f = set_aspect.clone();
        self.crop.aspect.connect_selected_notify(move |_| f());
        let f = set_aspect.clone();
        self.crop.custom_w.connect_value_changed(move |_| f());
        self.crop
            .custom_h
            .connect_value_changed(move |_| set_aspect());
        let weak = Rc::downgrade(self);
        apply.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.command(Command::ApplyCrop);
            }
        });
        let weak = Rc::downgrade(self);
        cancel.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                ui.command(Command::CancelCrop);
            }
        });
    }

    fn draw(self: &Rc<Self>, cr: &cairo::Context, w: i32, h: i32) {
        let Some((frame, labels, busy)) = self.edit(|editor| {
            if editor.view().viewport != (f64::from(w), f64::from(h)) {
                editor.set_viewport(f64::from(w), f64::from(h));
            }
            editor.poll_background();
            (editor.frame(), editor.labels(), editor.background_busy())
        }) else {
            return;
        };
        if busy {
            self.poll_background();
        }
        let (fw, fh) = frame.dimensions();
        let mut data = Vec::with_capacity(frame.as_raw().len());
        for p in frame.pixels() {
            let [r, g, b, a] = p.0;
            data.extend_from_slice(&[b, g, r, a]);
        }
        let stride = (fw as i32).saturating_mul(4);
        if let Ok(surface) = cairo::ImageSurface::create_for_data(
            data,
            cairo::Format::ARgb32,
            fw as i32,
            fh as i32,
            stride,
        ) {
            let _ = cr.set_source_surface(&surface, 0.0, 0.0);
            let _ = cr.paint();
        }
        // PROJ-005: label placeholders of missing sources.
        cr.set_font_size(14.0);
        for (p, text) in labels {
            if let Ok(extents) = cr.text_extents(&text) {
                let x = p.x - extents.width() / 2.0;
                let y = p.y + extents.height() / 2.0;
                cr.set_source_rgba(0.0, 0.0, 0.0, 0.7);
                cr.rectangle(
                    x - 6.0,
                    y - extents.height() - 6.0,
                    extents.width() + 12.0,
                    extents.height() + 12.0,
                );
                let _ = cr.fill();
                cr.set_source_rgb(1.0, 1.0, 1.0);
                cr.move_to(x, y);
                let _ = cr.show_text(&text);
            }
        }
    }

    /// UI-009: redraws as background resamples arrive, until none are left.
    fn poll_background(self: &Rc<Self>) {
        if self.polling.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        glib::timeout_add_local(std::time::Duration::from_millis(15), move || {
            let Some(ui) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let Some((arrived, busy)) =
                ui.edit(|editor| (editor.poll_background(), editor.background_busy()))
            else {
                // try again on the next tick
                return glib::ControlFlow::Continue;
            };
            if arrived {
                ui.canvas.queue_draw();
            }
            if busy {
                glib::ControlFlow::Continue
            } else {
                ui.polling.set(false);
                glib::ControlFlow::Break
            }
        });
    }

    fn update_cursor(&self) {
        let Some(cursor) = self.read(|e| e.cursor_at(self.pointer.get())) else {
            return;
        };
        self.canvas.set_cursor_from_name(Some(cursor_name(cursor)));
    }

    /// Runs `f` on the editor. Returns `None`, doing nothing, if the editor is already
    /// borrowed, which only happens when a GTK signal re-enters a handler (RUST-001).
    pub(crate) fn edit<R>(&self, f: impl FnOnce(&mut Editor) -> R) -> Option<R> {
        match self.editor.try_borrow_mut() {
            Ok(mut editor) => Some(f(&mut editor)),
            Err(_) => None,
        }
    }

    /// Like [`Ui::edit`], for reading.
    pub(crate) fn read<R>(&self, f: impl FnOnce(&Editor) -> R) -> Option<R> {
        match self.editor.try_borrow() {
            Ok(editor) => Some(f(&editor)),
            Err(_) => None,
        }
    }

    /// Runs a command, opening a dialog for those that need one.
    pub(crate) fn command(self: &Rc<Self>, command: Command) {
        if command.needs_ui() {
            dialogs::run(self, command);
        } else {
            self.edit(|e| e.execute(command));
            self.refresh();
        }
    }

    pub(crate) fn error(&self, message: &str) {
        gtk4::AlertDialog::builder()
            .message("Something went wrong")
            .detail(message)
            .modal(true)
            .build()
            .show(Some(&self.window));
    }

    fn refresh_crop_bar(&self) {
        let Some(crop) = self.read(|e| e.crop_tool().copied()) else {
            return;
        };
        self.crop.container.set_visible(crop.is_some());
        let Some(crop) = crop else { return };
        self.updating.set(true);
        let r = crop.pixel_rect();
        let lim = crop.limits;
        self.crop.left.set_range(
            f64::from(lim.left),
            f64::from(lim.right().saturating_sub(1)),
        );
        self.crop.top.set_range(
            f64::from(lim.top),
            f64::from(lim.bottom().saturating_sub(1)),
        );
        self.crop.width.set_range(1.0, f64::from(lim.width));
        self.crop.height.set_range(1.0, f64::from(lim.height));
        self.crop.left.set_value(f64::from(r.left));
        self.crop.top.set_value(f64::from(r.top));
        self.crop.width.set_value(f64::from(r.width));
        self.crop.height.set_value(f64::from(r.height));
        let index = AspectChoice::PRESETS
            .iter()
            .position(|a| *a == crop.aspect)
            .unwrap_or(AspectChoice::PRESETS.len());
        self.crop.aspect.set_selected(index as u32);
        let custom = matches!(crop.aspect, AspectChoice::Custom(..));
        self.crop.custom_w.set_visible(custom);
        self.crop.custom_h.set_visible(custom);
        self.updating.set(false);
    }

    /// Brings every widget up to date with the editor.
    pub(crate) fn refresh(&self) {
        let message = self.edit(|e| e.take_message()).flatten();
        if let Some(message) = message {
            self.error(&message);
        }
        self.edit(|editor| {
            self.window.set_title(Some(&editor.title()));
            self.save.set_sensitive(!editor.is_empty());
            self.undo.set_sensitive(editor.can_undo());
            self.redo.set_sensitive(editor.can_redo());
            self.undo.set_tooltip_text(Some(
                &editor
                    .undo_label()
                    .unwrap_or_else(|| "Nothing to undo".to_owned()),
            ));
            self.redo.set_tooltip_text(Some(
                &editor
                    .redo_label()
                    .unwrap_or_else(|| "Nothing to redo".to_owned()),
            ));
            self.status.set_text(&editor.canvas_text());
            self.zoom.set_label(&editor.zoom_text());
            match editor.exif_notice() {
                Some(text) => {
                    self.exif.set_label(&format!("ⓘ {text}"));
                    self.exif.set_visible(true);
                }
                None => self.exif.set_visible(false),
            }
            let missing = editor.missing_sources();
            self.missing.set_visible(!missing.is_empty());
            if let Some((_, name, _)) = missing.first() {
                let text = if missing.len() == 1 {
                    format!("{name} is missing")
                } else {
                    format!("{} source images are missing", missing.len())
                };
                self.missing_label.set_text(&text);
            }
            self.hint.set_visible(editor.is_empty());
        });
        self.refresh_crop_bar();
        self.refresh_smear_bar();
        self.refresh_light_bar();
        self.refresh_dodge_bar();
        let model = self.menu_model();
        self.menu
            .set_popover(Some(&PopoverMenu::from_model(Some(&model))));
        if let Some(Ok(action)) = self
            .window
            .lookup_action("undo")
            .map(|a| a.downcast::<gio::SimpleAction>())
            && let Some(enabled) = self.read(|e| e.can_undo())
        {
            action.set_enabled(enabled);
        }
        if let Some(Ok(action)) = self
            .window
            .lookup_action("redo")
            .map(|a| a.downcast::<gio::SimpleAction>())
            && let Some(enabled) = self.read(|e| e.can_redo())
        {
            action.set_enabled(enabled);
        }
        self.canvas.queue_draw();
    }
}

#[cfg(test)]
mod tests {
    //! GTK wiring tests. They need a display and are skipped without one. All GTK calls
    //! stay on this one test's thread.

    use stitchpane_core::codec::{self, EncodeOptions, Format};
    use stitchpane_core::model::{ItemId, Step};

    use super::*;

    fn write(dir: &std::path::Path, name: &str, exif: Option<Vec<u8>>) -> PathBuf {
        let path = dir.join(name);
        let pixels = image::RgbaImage::from_pixel(60, 40, image::Rgba([10, 200, 30, 255]));
        let options = EncodeOptions {
            format: Format::from_path(&path).expect("format"),
            quality: 90,
            dpi: None,
            exif,
        };
        codec::write(&pixels, &path, &options).expect("write");
        path
    }

    /// Big-endian EXIF block with Make = "Cam".
    fn exif_make() -> Vec<u8> {
        let mut b = b"MM\0\x2a\0\0\0\x08".to_vec();
        b.extend_from_slice(&[0, 1, 0x01, 0x0F, 0, 2, 0, 0, 0, 4]);
        b.extend_from_slice(b"Cam\0");
        b.extend_from_slice(&[0, 0, 0, 0]);
        b
    }

    #[test]
    fn widgets_follow_the_editor() {
        if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
            return;
        }
        if let Err(error) = gtk4::init() {
            eprintln!("skipping: GTK did not initialize: {error}");
            return;
        }
        let dir = std::env::temp_dir().join(format!("stitchpane-gtk-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let plain = write(&dir, "plain.png", None);
        let photo = write(&dir, "photo.jpg", Some(exif_make()));
        let app = Application::builder()
            .application_id("app.stitchpane.Test")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .build();
        app.register(None::<&gio::Cancellable>).expect("register");

        let ui = Ui::new(&app, vec![plain.clone()]);

        // spec: UNDO-003 — the undo button names the step.
        assert!(ui.editor.borrow_mut().apply(Step::MoveItem {
            item: ItemId(1),
            x: 3,
            y: 0
        }));
        ui.refresh();
        assert_eq!(ui.undo.tooltip_text().as_deref(), Some("Undo Move image"));
        assert!(!ui.redo.is_sensitive());

        // spec: CROP-002, CROP-004 — the crop bar shows and edits the rectangle.
        ui.command(Command::Crop(ItemId(1)));
        assert!(ui.crop.container.is_visible());
        assert_eq!(ui.crop.width.value(), 60.0);
        ui.crop.width.set_value(30.0);
        assert_eq!(
            ui.editor.borrow().crop_tool().map(|c| c.pixel_rect().width),
            Some(30)
        );
        ui.command(Command::ApplyCrop);
        assert!(!ui.crop.container.is_visible());
        assert_eq!(
            ui.editor
                .borrow()
                .project()
                .state()
                .item(ItemId(1))
                .map(|i| i.width),
            Some(30)
        );

        // spec: UI-010 — the zoom level in the status bar opens the zoom choices.
        let zoom_label = ui.editor.borrow().zoom_text();
        assert_eq!(ui.zoom.label().as_deref(), Some(zoom_label.as_str()));
        assert_eq!(ui.zoom.menu_model().map(|m| m.n_items()), Some(4));

        // spec: SMEAR-001 — the smear bar shows the brush and Done leaves the tool.
        ui.command(Command::Smear(ItemId(1)));
        assert!(ui.smear.container.is_visible());
        assert_eq!(ui.smear.strength.value(), 50.0);
        ui.smear.size.set_value(12.0);
        assert_eq!(ui.editor.borrow().smear_tool().map(|t| t.size), Some(12.0));
        ui.command(Command::EndSmear);
        assert!(!ui.smear.container.is_visible());

        // spec: LIGHT-001 — the light bar edits the settings and Apply records them.
        ui.command(Command::AdjustLight(ItemId(1)));
        assert!(ui.light.container.is_visible());
        assert_eq!(ui.light.white.value(), 255.0);
        ui.light.exposure.set_value(1.0);
        assert_eq!(
            ui.editor.borrow().light_tool().map(|t| t.light.exposure),
            Some(1.0)
        );
        ui.light.black.set_value(30.0);
        assert_eq!(ui.light.white.adjustment().lower(), 31.0);
        ui.command(Command::ApplyLight);
        assert!(!ui.light.container.is_visible());
        assert_eq!(
            ui.editor
                .borrow()
                .project()
                .state()
                .item(ItemId(1))
                .and_then(|i| i.light)
                .map(|l| (l.exposure, l.black)),
            Some((1.0, 30.0))
        );

        // spec: DODGE-001 — the dodge and burn bar shows the brush and Done leaves the tool.
        ui.command(Command::DodgeBurn(ItemId(1)));
        assert!(ui.dodge.container.is_visible());
        assert_eq!(ui.dodge.strength.value(), 50.0);
        ui.dodge.range.set_selected(0);
        ui.dodge.size.set_value(9.0);
        let tool = ui.editor.borrow().dodge_tool();
        assert_eq!(
            tool.map(|t| (t.size, t.range)),
            Some((9.0, ToneRange::Shadows))
        );
        ui.command(Command::EndDodgeBurn);
        assert!(!ui.dodge.container.is_visible());

        // spec: META-001 — the status bar shows the EXIF notice.
        assert!(!ui.exif.is_visible());
        ui.editor.borrow_mut().open(&photo).expect("open photo");
        ui.refresh();
        assert!(ui.exif.is_visible());
        assert_eq!(ui.exif.label().as_deref(), Some("ⓘ Contains EXIF metadata"));

        // spec: PROJ-005 — missing sources are reported with a way to locate them.
        let project = dir.join("p.stitchpane");
        ui.editor.borrow_mut().save_as(&project).expect("save");

        // spec: UI-008 — the header bar's Save button saves to the project's file.
        assert!(ui.save.is_sensitive());
        ui.editor.borrow_mut().strip_metadata(ItemId(1));
        assert!(ui.editor.borrow().is_dirty());
        ui.save.emit_clicked();
        while glib::MainContext::default().iteration(false) {}
        assert!(!ui.editor.borrow().is_dirty());
        std::fs::remove_file(&photo).expect("delete");
        ui.editor.borrow_mut().open(&project).expect("open project");
        ui.refresh();
        assert!(ui.missing.is_visible());
        assert_eq!(ui.missing_label.text(), "photo.jpg is missing");

        ui.editor.replace(Editor::new());
        ui.window.destroy();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
