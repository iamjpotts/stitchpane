//! iced user interface. All editing behavior lives in [`imaginer_core::editor::Editor`];
//! this crate shows its frame, forwards input to it, and provides the dialogs.

mod dialogs;

use std::path::PathBuf;
use std::process::ExitCode;

use dialogs::Modal;
use iced::keyboard::{self, key::Named};
use iced::mouse::{self, ScrollDelta};
use iced::widget::canvas::{self, Canvas, Frame, Geometry, Program};
use iced::widget::{
    Space, button, column, container, image, mouse_area, opaque, pick_list, pin, row, stack, text,
    text_input, tooltip,
};
use iced::{
    Element, Event, Length, Point as IPoint, Rectangle, Renderer, Size, Task, Theme, window,
};
use imaginer_core::editor::{
    AspectChoice, Button as PointerButton, Command, Cursor, Editor, Key, KeyOutcome, MenuEntry,
    Modifiers,
};
use imaginer_core::geometry::{PixelRect, Point};

pub fn run() -> ExitCode {
    let paths: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let result = iced::application(move || App::new(paths.clone()), App::update, App::view)
        .title(App::title)
        .subscription(App::subscription)
        .exit_on_close_request(false)
        .window_size((1200.0, 800.0))
        .run();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Clone, Debug)]
pub enum Message {
    Resized(Size),
    PointerDown(IPoint, PointerButton),
    PointerMove(IPoint),
    PointerUp(IPoint),
    Scroll(IPoint, f32, f32),
    Key(keyboard::Key, keyboard::Modifiers),
    Modifiers(keyboard::Modifiers),
    Command(Command),
    CloseMenu,
    FileDropped(PathBuf),
    CloseRequested(window::Id),
    Modal(dialogs::ModalMessage),
    /// Files chosen in a file dialog, for a purpose.
    Picked(dialogs::Purpose, Vec<PathBuf>),
    CropField(usize, String),
    CropAspect(String),
    CropCustom(usize, String),
    Exif,
    Locate,
    OpenMenu(MenuKind),
}

/// The main menus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuKind {
    File,
    Edit,
    View,
    Image,
}

impl MenuKind {
    const ALL: [MenuKind; 4] = [
        MenuKind::File,
        MenuKind::Edit,
        MenuKind::View,
        MenuKind::Image,
    ];

    fn label(self) -> &'static str {
        match self {
            MenuKind::File => "File",
            MenuKind::Edit => "Edit",
            MenuKind::View => "View",
            MenuKind::Image => "Image",
        }
    }
}

/// Choices in the crop bar's aspect list (CROP-003).
const ASPECTS: [&str; 7] = ["Free", "Original", "1:1", "4:3", "3:2", "16:9", "Custom"];

pub struct App {
    editor: Editor,
    frame: Option<image::Handle>,
    frame_size: Size,
    labels: Vec<(Point, String)>,
    canvas_size: Size,
    modifiers: keyboard::Modifiers,
    pointer: IPoint,
    cursor: Cursor,
    menu: Option<(IPoint, Vec<MenuEntry>)>,
    modal: Option<Modal>,
    crop_fields: [String; 4],
    custom_aspect: [String; 2],
    window: Option<window::Id>,
    /// Status bar notices, computed in refresh() (META-001, PROJ-005).
    exif_notice: Option<String>,
    missing_notice: Option<String>,
}

fn modifiers(m: keyboard::Modifiers) -> Modifiers {
    Modifiers {
        shift: m.shift(),
        ctrl: m.control(),
        alt: m.alt(),
    }
}

fn point(p: IPoint) -> Point {
    Point::new(f64::from(p.x), f64::from(p.y))
}

fn map_key(key: &keyboard::Key) -> Option<Key> {
    Some(match key.as_ref() {
        keyboard::Key::Named(Named::ArrowLeft) => Key::Left,
        keyboard::Key::Named(Named::ArrowRight) => Key::Right,
        keyboard::Key::Named(Named::ArrowUp) => Key::Up,
        keyboard::Key::Named(Named::ArrowDown) => Key::Down,
        keyboard::Key::Named(Named::Escape) => Key::Escape,
        keyboard::Key::Named(Named::Enter) => Key::Enter,
        keyboard::Key::Character(c) => match c {
            "[" | "{" => Key::BracketLeft,
            "]" | "}" => Key::BracketRight,
            "+" | "=" => Key::Plus,
            "-" | "_" => Key::Minus,
            other => Key::Char(other.chars().next()?.to_ascii_lowercase()),
        },
        _ => return None,
    })
}

fn interaction(cursor: Cursor) -> mouse::Interaction {
    match cursor {
        Cursor::Default => mouse::Interaction::Idle,
        Cursor::Move => mouse::Interaction::Move,
        Cursor::Grab => mouse::Interaction::Grab,
        Cursor::ResizeHorizontal => mouse::Interaction::ResizingHorizontally,
        Cursor::ResizeVertical => mouse::Interaction::ResizingVertically,
        Cursor::ResizeDiagonal => mouse::Interaction::ResizingDiagonallyDown,
        Cursor::Rotate => mouse::Interaction::Crosshair,
    }
}

impl App {
    fn new(paths: Vec<PathBuf>) -> (Self, Task<Message>) {
        let mut app = Self {
            editor: Editor::new(),
            frame: None,
            frame_size: Size::ZERO,
            labels: Vec::new(),
            canvas_size: Size::ZERO,
            modifiers: keyboard::Modifiers::default(),
            pointer: IPoint::ORIGIN,
            cursor: Cursor::Default,
            menu: None,
            modal: None,
            crop_fields: Default::default(),
            custom_aspect: ["3".to_owned(), "4".to_owned()],
            window: None,
            exif_notice: None,
            missing_notice: None,
        };
        let result = match paths.len() {
            0 => Ok(()),
            1 => app.editor.open(&paths[0]),
            _ => app.editor.open_images(&paths),
        };
        if let Err(e) = result {
            app.modal = Some(Modal::Error(e));
        }
        app.refresh();
        (app, Task::none())
    }

    fn title(&self) -> String {
        self.editor.title()
    }

    /// Re-renders the frame and syncs the crop fields after any change.
    fn refresh(&mut self) {
        if let Some(message) = self.editor.take_message() {
            self.modal = Some(Modal::Error(message));
        }
        if self.canvas_size.width >= 1.0 && self.canvas_size.height >= 1.0 {
            let frame = self.editor.frame();
            let (w, h) = frame.dimensions();
            self.frame_size = Size::new(w as f32, h as f32);
            self.frame = Some(image::Handle::from_rgba(w, h, frame.into_raw()));
            self.labels = self.editor.labels();
        }
        self.exif_notice = self.editor.exif_notice();
        let missing = self.editor.missing_sources();
        self.missing_notice = match missing.len() {
            0 => None,
            1 => Some(format!("{} is missing", missing[0].1)),
            n => Some(format!("{n} source images are missing")),
        };
        if let Some(crop) = self.editor.crop_tool() {
            let r = crop.pixel_rect();
            self.crop_fields = [r.left, r.top, r.width, r.height].map(|v| v.to_string());
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        let task = self.handle(message);
        self.refresh();
        task
    }

    fn handle(&mut self, message: Message) -> Task<Message> {
        let mods = modifiers(self.modifiers);
        match message {
            Message::Resized(size) => {
                self.canvas_size = size;
                self.editor
                    .set_viewport(f64::from(size.width), f64::from(size.height));
            }
            Message::PointerDown(p, button) => {
                self.menu = None;
                self.pointer = p;
                if let Some(menu) = self.editor.pointer_down(point(p), button, mods) {
                    self.menu = Some((p, menu));
                }
            }
            Message::PointerMove(p) => {
                self.pointer = p;
                self.editor.pointer_move(point(p), mods);
                self.cursor = self.editor.cursor_at(point(p));
            }
            Message::PointerUp(p) => self.editor.pointer_up(point(p), mods),
            Message::Scroll(p, dx, dy) => {
                self.editor
                    .scroll(point(p), f64::from(dx), f64::from(dy), mods)
            }
            Message::Modifiers(m) => self.modifiers = m,
            Message::Key(key, m) => {
                self.modifiers = m;
                if self.modal.is_some() {
                    if key == keyboard::Key::Named(Named::Escape) {
                        self.modal = None;
                    }
                    return Task::none();
                }
                if let Some(key) = map_key(&key)
                    && let KeyOutcome::Command(command) = self.editor.key_press(key, modifiers(m))
                {
                    return self.command(command);
                }
            }
            Message::Command(command) => {
                self.menu = None;
                return self.command(command);
            }
            Message::CloseMenu => self.menu = None,
            Message::FileDropped(path) => {
                let at = point(self.pointer);
                if let Err(e) = self.editor.add_images(&[path], Some(at)) {
                    self.modal = Some(Modal::Error(e));
                }
            }
            Message::CloseRequested(id) => {
                self.window = Some(id);
                if self.editor.needs_save_prompt() {
                    self.modal = Some(Modal::ConfirmDiscard(dialogs::AfterDiscard::Quit));
                } else {
                    return window::close(id);
                }
            }
            Message::Modal(message) => return dialogs::update(self, message),
            Message::Picked(purpose, paths) => return dialogs::picked(self, purpose, paths),
            Message::CropField(index, value) => {
                self.crop_fields[index] = value;
                let parsed: Vec<u32> = self
                    .crop_fields
                    .iter()
                    .filter_map(|v| v.trim().parse().ok())
                    .collect();
                if let [l, t, w, h] = parsed[..] {
                    self.editor
                        .set_crop_rect(PixelRect::new(l, t, w.max(1), h.max(1)));
                }
                return Task::none();
            }
            Message::CropAspect(label) => {
                let aspect = match ASPECTS.iter().position(|a| *a == label) {
                    Some(i) if i < AspectChoice::PRESETS.len() => AspectChoice::PRESETS[i],
                    _ => self.custom_aspect(),
                };
                self.editor.set_crop_aspect(aspect);
            }
            Message::CropCustom(index, value) => {
                self.custom_aspect[index] = value;
                let aspect = self.custom_aspect();
                self.editor.set_crop_aspect(aspect);
            }
            Message::Exif => {
                let item = self.editor.exif_viewer_item();
                return self.command(Command::ExifViewer(item));
            }
            Message::OpenMenu(kind) => {
                let index = MenuKind::ALL.iter().position(|k| *k == kind).unwrap_or(0);
                let x = 6.0 + index as f32 * 58.0;
                self.menu = Some((IPoint::new(x, 0.0), self.menu_entries(kind)));
            }
            Message::Locate => {
                if let Some((source, ..)) = self.editor.missing_sources().first() {
                    return self.command(Command::LocateSource(*source));
                }
            }
        }
        Task::none()
    }

    fn custom_aspect(&self) -> AspectChoice {
        let [w, h] = &self.custom_aspect;
        AspectChoice::Custom(
            w.trim().parse().unwrap_or(1).max(1),
            h.trim().parse().unwrap_or(1).max(1),
        )
    }

    fn command(&mut self, command: Command) -> Task<Message> {
        if command.needs_ui() {
            dialogs::run(self, command)
        } else {
            self.editor.execute(command);
            Task::none()
        }
    }

    fn subscription(&self) -> iced::Subscription<Message> {
        iced::event::listen_with(|event, status, id| match event {
            Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. })
                if status == iced::event::Status::Ignored =>
            {
                Some(Message::Key(key, modifiers))
            }
            Event::Keyboard(keyboard::Event::ModifiersChanged(m)) => Some(Message::Modifiers(m)),
            Event::Window(window::Event::FileDropped(path)) => Some(Message::FileDropped(path)),
            Event::Window(window::Event::CloseRequested) => Some(Message::CloseRequested(id)),
            _ => None,
        })
    }

    fn header(&self) -> Element<'_, Message> {
        let tip = |content: Element<'static, Message>, label: String| {
            tooltip(
                content,
                container(text(label))
                    .padding(6)
                    .style(container::rounded_box),
                tooltip::Position::Bottom,
            )
        };
        let action = |label: &'static str, command: Command| {
            button(text(label).size(14)).on_press(Message::Command(command))
        };
        let undo = button(text("Undo").size(14)).on_press_maybe(
            self.editor
                .can_undo()
                .then_some(Message::Command(Command::Undo)),
        );
        let redo = button(text("Redo").size(14)).on_press_maybe(
            self.editor
                .can_redo()
                .then_some(Message::Command(Command::Redo)),
        );
        let undo_label = self
            .editor
            .undo_label()
            .unwrap_or_else(|| "Nothing to undo".to_owned());
        let redo_label = self
            .editor
            .redo_label()
            .unwrap_or_else(|| "Nothing to redo".to_owned());
        let menus = MenuKind::ALL.map(|kind| {
            button(text(kind.label()).size(14))
                .style(button::text)
                .on_press(Message::OpenMenu(kind))
                .into()
        });
        let mut bar = row(menus)
            .spacing(2)
            .padding(6)
            .align_y(iced::Alignment::Center);
        bar = bar
            .push(Space::new().width(12))
            .push(action("Open…", Command::Open))
            .push(action("Add images…", Command::AddImages))
            .push(tip(undo.into(), undo_label))
            .push(tip(redo.into(), redo_label))
            .push(Space::new().width(Length::Fill))
            .push(action("−", Command::ZoomOut))
            .push(action("Fit", Command::ZoomFit))
            .push(action("100%", Command::Zoom100))
            .push(action("+", Command::ZoomIn));
        bar.spacing(6).into()
    }

    /// Entries of a main menu (the Edit menu names the undo and redo steps, UNDO-003).
    fn menu_entries(&self, kind: MenuKind) -> Vec<MenuEntry> {
        let entry = |label: String, command: Command| MenuEntry {
            label,
            command: Some(command),
        };
        match kind {
            MenuKind::File => vec![
                entry("Open…  (Ctrl+O)".into(), Command::Open),
                entry("Add images…".into(), Command::AddImages),
                entry("Save  (Ctrl+S)".into(), Command::Save),
                entry("Save as…  (Ctrl+Shift+S)".into(), Command::SaveAs),
                entry("Export…  (Ctrl+E)".into(), Command::Export),
            ],
            MenuKind::Edit => vec![
                entry(
                    format!(
                        "{}  (Ctrl+Z)",
                        self.editor.undo_label().unwrap_or_else(|| "Undo".into())
                    ),
                    Command::Undo,
                ),
                entry(
                    format!(
                        "{}  (Ctrl+Shift+Z)",
                        self.editor.redo_label().unwrap_or_else(|| "Redo".into())
                    ),
                    Command::Redo,
                ),
            ],
            MenuKind::View => vec![
                entry("Zoom in  (Ctrl++)".into(), Command::ZoomIn),
                entry("Zoom out  (Ctrl+−)".into(), Command::ZoomOut),
                entry("Fit  (Ctrl+0)".into(), Command::ZoomFit),
                entry("100%  (Ctrl+1)".into(), Command::Zoom100),
                entry(
                    if self.editor.snapping() {
                        "Turn snapping off".into()
                    } else {
                        "Turn snapping on".into()
                    },
                    Command::ToggleSnapping,
                ),
            ],
            MenuKind::Image => vec![
                entry("Canvas size…".into(), Command::CanvasSize),
                entry("Set DPI…".into(), Command::SetDpi),
                entry("Show EXIF metadata…".into(), Command::ExifViewer(None)),
            ],
        }
    }

    fn crop_bar(&self) -> Option<Element<'_, Message>> {
        let crop = self.editor.crop_tool()?;
        let field = |index: usize, label: &'static str| {
            row![
                text(label),
                text_input("", &self.crop_fields[index])
                    .on_input(move |v| Message::CropField(index, v))
                    .width(70)
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center)
        };
        let selected = AspectChoice::PRESETS
            .iter()
            .position(|a| *a == crop.aspect)
            .map(|i| ASPECTS[i])
            .unwrap_or("Custom");
        let aspect = pick_list(
            ASPECTS.map(str::to_owned).to_vec(),
            Some(selected.to_owned()),
            Message::CropAspect,
        );
        let mut bar = row![
            field(0, "Left"),
            field(1, "Top"),
            field(2, "Width"),
            field(3, "Height"),
            text("Aspect"),
            aspect
        ]
        .spacing(10)
        .padding(6)
        .align_y(iced::Alignment::Center);
        if selected == "Custom" {
            bar = bar.push(
                row![
                    text_input("", &self.custom_aspect[0])
                        .on_input(|v| Message::CropCustom(0, v))
                        .width(50),
                    text(":"),
                    text_input("", &self.custom_aspect[1])
                        .on_input(|v| Message::CropCustom(1, v))
                        .width(50)
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
            );
        }
        bar = bar
            .push(Space::new().width(Length::Fill))
            .push(button(text("Cancel")).on_press(Message::Command(Command::CancelCrop)))
            .push(
                button(text("Apply"))
                    .style(button::success)
                    .on_press(Message::Command(Command::ApplyCrop)),
            );
        Some(bar.into())
    }

    fn view(&self) -> Element<'_, Message> {
        let canvas = Canvas::new(CanvasView { app: self })
            .width(Length::Fill)
            .height(Length::Fill);
        let mut layers = stack![canvas];
        if self.editor.is_empty() {
            layers = layers.push(
                container(text("Open an image or drop image files here").size(24))
                    .center(Length::Fill),
            );
        }
        if let Some((p, entries)) = &self.menu {
            layers = layers.push(
                mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
                    .on_press(Message::CloseMenu)
                    .on_right_press(Message::CloseMenu),
            );
            // Keep the menu inside the canvas area.
            let separators = entries.iter().filter(|e| e.command.is_none()).count() as f32;
            let height = (entries.len() as f32 - separators) * 30.0 + separators * 9.0 + 8.0;
            let x = p.x.min(self.canvas_size.width - MENU_WIDTH).max(0.0);
            let y = p.y.min(self.canvas_size.height - height).max(0.0);
            layers = layers.push(pin(context_menu(entries)).x(x).y(y));
        }
        let mut page = column![self.header()];
        if let Some(bar) = self.crop_bar() {
            page = page.push(bar);
        }
        page = page.push(layers);
        page = page.push(dialogs::status(self));
        let page: Element<'_, Message> = page.into();
        match &self.modal {
            Some(modal) => stack![
                page,
                opaque(
                    container(dialogs::view(self, modal))
                        .center(Length::Fill)
                        .style(|_| container::Style {
                            background: Some(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.4).into()),
                            ..container::Style::default()
                        })
                )
            ]
            .into(),
            None => page,
        }
    }
}

const MENU_WIDTH: f32 = 260.0;

fn context_menu(entries: &[MenuEntry]) -> Element<'_, Message> {
    let mut list = column![].spacing(0).width(MENU_WIDTH);
    for entry in entries {
        list = match &entry.command {
            None => list.push(iced::widget::rule::horizontal(1)),
            Some(command) => list.push(
                button(text(entry.label.clone()))
                    .width(Length::Fill)
                    .style(button::text)
                    .on_press(Message::Command(command.clone())),
            ),
        };
    }
    container(list)
        .padding(4)
        .style(container::bordered_box)
        .into()
}

/// The canvas: shows the editor's frame and turns mouse input into messages.
struct CanvasView<'a> {
    app: &'a App,
}

#[derive(Default)]
struct CanvasState {
    size: Size,
    pressed: bool,
}

impl Program<Message> for CanvasView<'_> {
    type State = CanvasState;

    fn update(
        &self,
        state: &mut CanvasState,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        if state.size != bounds.size() {
            state.size = bounds.size();
            return Some(canvas::Action::publish(Message::Resized(bounds.size())));
        }
        let Event::Mouse(event) = event else {
            return None;
        };
        let inside = cursor.position_in(bounds);
        let relative = cursor
            .position()
            .map(|p| IPoint::new(p.x - bounds.x, p.y - bounds.y));
        let message = match event {
            mouse::Event::ButtonPressed(mouse::Button::Left) => {
                let p = inside?;
                state.pressed = true;
                Message::PointerDown(p, PointerButton::Primary)
            }
            mouse::Event::ButtonPressed(mouse::Button::Right) => {
                Message::PointerDown(inside?, PointerButton::Secondary)
            }
            mouse::Event::CursorMoved { .. } => {
                let p = if state.pressed { relative? } else { inside? };
                Message::PointerMove(p)
            }
            mouse::Event::ButtonReleased(mouse::Button::Left) if state.pressed => {
                state.pressed = false;
                Message::PointerUp(relative?)
            }
            mouse::Event::WheelScrolled { delta } => {
                let (dx, dy) = match *delta {
                    ScrollDelta::Lines { x, y } => (-x, -y),
                    ScrollDelta::Pixels { x, y } => (-x / 40.0, -y / 40.0),
                };
                Message::Scroll(inside?, dx, dy)
            }
            _ => return None,
        };
        Some(canvas::Action::publish(message).and_capture())
    }

    fn draw(
        &self,
        _state: &CanvasState,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        if let Some(handle) = &self.app.frame {
            frame.draw_image(
                Rectangle::new(IPoint::ORIGIN, self.app.frame_size),
                canvas::Image::new(handle.clone()),
            );
        }
        // PROJ-005: label placeholders of missing sources.
        for (p, label) in &self.app.labels {
            frame.fill_text(canvas::Text {
                content: label.clone(),
                position: IPoint::new(p.x as f32, p.y as f32),
                color: iced::Color::WHITE,
                size: 16.into(),
                align_x: iced::alignment::Horizontal::Center.into(),
                align_y: iced::alignment::Vertical::Center,
                ..canvas::Text::default()
            });
        }
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        _state: &CanvasState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if cursor.is_over(bounds) {
            interaction(self.app.cursor)
        } else {
            mouse::Interaction::default()
        }
    }
}

#[cfg(test)]
mod tests;
