//! GTK 4 user interface. All editing behavior lives in [`imaginer_core::editor::Editor`];
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
use imaginer_core::editor::{
    AspectChoice, Button as PointerButton, Command, Cursor, Editor, Key, KeyOutcome, Modifiers,
};
use imaginer_core::geometry::{PixelRect, Point};
use imaginer_core::{APP_ID, APP_NAME};

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

pub(crate) struct Ui {
    editor: RefCell<Editor>,
    window: ApplicationWindow,
    canvas: DrawingArea,
    undo: Button,
    redo: Button,
    menu: MenuButton,
    status: Label,
    exif: Button,
    missing: GBox,
    missing_label: Label,
    hint: Label,
    crop: CropBar,
    /// Last pointer position over the canvas.
    pointer: Cell<Point>,
    drag_start: Cell<Point>,
    /// Suppresses control callbacks while refresh() updates them.
    updating: Cell<bool>,
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
    }
}

fn spin(max: f64) -> SpinButton {
    let s = SpinButton::with_range(0.0, max, 1.0);
    s.set_width_chars(6);
    s
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
        let undo = Button::from_icon_name("edit-undo-symbolic");
        let redo = Button::from_icon_name("edit-redo-symbolic");
        header.pack_start(&open);
        header.pack_start(&add);
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

        let status_bar = GBox::new(GOrientation::Horizontal, 12);
        status_bar.set_margin_start(8);
        status_bar.set_margin_end(8);
        status_bar.set_margin_top(2);
        status_bar.set_margin_bottom(2);
        let status = Label::new(None);
        status.set_halign(Align::Start);
        status.set_hexpand(true);
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
        status_bar.append(&missing);
        status_bar.append(&exif);

        let layout = GBox::new(GOrientation::Vertical, 0);
        layout.append(&crop.container);
        layout.append(&overlay);
        layout.append(&status_bar);
        window.set_child(Some(&layout));

        let ui = Rc::new(Ui {
            editor: RefCell::new(Editor::new()),
            window: window.clone(),
            canvas: canvas.clone(),
            undo: undo.clone(),
            redo: redo.clone(),
            menu,
            status,
            exif: exif.clone(),
            missing,
            missing_label,
            hint,
            crop,
            pointer: Cell::new(Point::default()),
            drag_start: Cell::new(Point::default()),
            updating: Cell::new(false),
        });

        // Callbacks hold weak references; the window owns the UI until it is destroyed.
        let keep = RefCell::new(Some(ui.clone()));
        window.connect_destroy(move |_| {
            keep.borrow_mut().take();
        });

        ui.install_actions();
        ui.connect_canvas();
        ui.connect_crop_bar(&apply_crop, &cancel_crop);

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
        undo.connect_clicked(on(Command::Undo));
        redo.connect_clicked(on(Command::Redo));
        let weak = Rc::downgrade(&ui);
        exif.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                let item = ui.editor.borrow_mut().exif_viewer_item();
                ui.command(Command::ExifViewer(item));
            }
        });
        let weak = Rc::downgrade(&ui);
        locate.connect_clicked(move |_| {
            if let Some(ui) = weak.upgrade() {
                let first = ui
                    .editor
                    .borrow_mut()
                    .missing_sources()
                    .first()
                    .map(|m| m.0);
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
            if !ui.editor.borrow().needs_save_prompt() {
                return glib::Propagation::Proceed;
            }
            let window = window.clone();
            glib::spawn_future_local(async move {
                if dialogs::confirm_discard(&ui).await {
                    ui.editor.replace(Editor::new());
                    window.destroy();
                }
            });
            glib::Propagation::Stop
        });

        window.present();
        if !paths.is_empty() {
            let result = if paths.len() == 1 {
                ui.editor.borrow_mut().open(&paths[0])
            } else {
                ui.editor.borrow_mut().open_images(&paths)
            };
            if let Err(e) = result {
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
                            Command::ExifViewer(ui.editor.borrow_mut().exif_viewer_item())
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
                action.set_state(&ui.editor.borrow().snapping().to_variant());
            }
        });
        self.window.add_action(&snapping);
    }

    /// The main menu, rebuilt on refresh so undo and redo show their step (UNDO-003).
    fn menu_model(&self) -> gio::Menu {
        let editor = self.editor.borrow();
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
            &editor.undo_label().unwrap_or_else(|| "Undo".to_owned()),
            "win.undo",
            Some("<Control>z"),
        ));
        edit.append_item(&item(
            &editor.redo_label().unwrap_or_else(|| "Redo".to_owned()),
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
                ui.editor
                    .borrow_mut()
                    .set_viewport(f64::from(w), f64::from(h));
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
            ui.editor
                .borrow_mut()
                .pointer_down(p, PointerButton::Primary, mods);
            ui.refresh();
        });
        let weak = Rc::downgrade(self);
        drag.connect_drag_update(move |gesture, dx, dy| {
            let Some(ui) = weak.upgrade() else { return };
            let s = ui.drag_start.get();
            let mods = modifiers(gesture.current_event_state());
            ui.editor
                .borrow_mut()
                .pointer_move(Point::new(s.x + dx, s.y + dy), mods);
            ui.update_cursor();
            ui.canvas.queue_draw();
            ui.refresh_crop_bar();
        });
        let weak = Rc::downgrade(self);
        drag.connect_drag_end(move |gesture, dx, dy| {
            let Some(ui) = weak.upgrade() else { return };
            let s = ui.drag_start.get();
            let mods = modifiers(gesture.current_event_state());
            ui.editor
                .borrow_mut()
                .pointer_up(Point::new(s.x + dx, s.y + dy), mods);
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
            let menu = ui.editor.borrow_mut().pointer_down(
                Point::new(x, y),
                PointerButton::Secondary,
                mods,
            );
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
            ui.editor.borrow_mut().scroll(p, dx, dy, mods);
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
            let result = ui
                .editor
                .borrow_mut()
                .add_images(&paths, Some(Point::new(x, y)));
            if let Err(e) = result {
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
            let outcome = ui.editor.borrow_mut().key_press(key, modifiers(state));
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
                ui.editor.borrow_mut().set_crop_rect(rect);
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
            ui.editor.borrow_mut().set_crop_aspect(aspect);
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

    fn draw(&self, cr: &cairo::Context, w: i32, h: i32) {
        let (frame, labels) = {
            let mut editor = self.editor.borrow_mut();
            if editor.view().viewport != (f64::from(w), f64::from(h)) {
                editor.set_viewport(f64::from(w), f64::from(h));
            }
            (editor.frame(), editor.labels())
        };
        let (fw, fh) = frame.dimensions();
        let mut data = Vec::with_capacity(frame.as_raw().len());
        for p in frame.pixels() {
            let [r, g, b, a] = p.0;
            data.extend_from_slice(&[b, g, r, a]);
        }
        let stride = fw as i32 * 4;
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

    fn update_cursor(&self) {
        let cursor = self.editor.borrow().cursor_at(self.pointer.get());
        self.canvas.set_cursor_from_name(Some(cursor_name(cursor)));
    }

    /// Runs a command, opening a dialog for those that need one.
    pub(crate) fn command(self: &Rc<Self>, command: Command) {
        if command.needs_ui() {
            dialogs::run(self, command);
        } else {
            self.editor.borrow_mut().execute(command);
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
        let editor = self.editor.borrow();
        let crop = editor.crop_tool().copied();
        self.crop.container.set_visible(crop.is_some());
        let Some(crop) = crop else { return };
        self.updating.set(true);
        let r = crop.pixel_rect();
        let lim = crop.limits;
        self.crop
            .left
            .set_range(f64::from(lim.left), f64::from(lim.right() - 1));
        self.crop
            .top
            .set_range(f64::from(lim.top), f64::from(lim.bottom() - 1));
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
        let message = self.editor.borrow_mut().take_message();
        if let Some(message) = message {
            self.error(&message);
        }
        {
            let mut editor = self.editor.borrow_mut();
            self.window.set_title(Some(&editor.title()));
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
            self.status.set_text(&editor.status_text());
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
        }
        self.refresh_crop_bar();
        let model = self.menu_model();
        self.menu
            .set_popover(Some(&PopoverMenu::from_model(Some(&model))));
        if let Some(action) = self.window.lookup_action("undo") {
            action
                .downcast::<gio::SimpleAction>()
                .expect("simple action")
                .set_enabled(self.editor.borrow().can_undo());
        }
        if let Some(action) = self.window.lookup_action("redo") {
            action
                .downcast::<gio::SimpleAction>()
                .expect("simple action")
                .set_enabled(self.editor.borrow().can_redo());
        }
        self.canvas.queue_draw();
    }
}

#[cfg(test)]
mod tests {
    //! GTK wiring tests. They need a display and are skipped without one. All GTK calls
    //! stay on this one test's thread.

    use imaginer_core::codec::{self, EncodeOptions, Format};
    use imaginer_core::model::{ItemId, Step};

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
        if gtk4::init().is_err() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("imaginer-gtk-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let plain = write(&dir, "plain.png", None);
        let photo = write(&dir, "photo.jpg", Some(exif_make()));
        let app = Application::builder()
            .application_id("app.imaginer.Test")
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

        // spec: META-001 — the status bar shows the EXIF notice.
        assert!(!ui.exif.is_visible());
        ui.editor.borrow_mut().open(&photo).expect("open photo");
        ui.refresh();
        assert!(ui.exif.is_visible());
        assert_eq!(ui.exif.label().as_deref(), Some("ⓘ Contains EXIF metadata"));

        // spec: PROJ-005 — missing sources are reported with a way to locate them.
        let project = dir.join("p.imaginer");
        ui.editor.borrow_mut().save_as(&project).expect("save");
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
