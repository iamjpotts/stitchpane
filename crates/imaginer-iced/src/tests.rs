//! UI tests driven through the headless iced simulator. Each test names the requirement it
//! verifies in a `spec:` comment.

use std::path::{Path, PathBuf};

use iced::{Size, Theme};
use iced_test::simulator;
use imaginer_core::codec::{self, EncodeOptions, Format};
use imaginer_core::model::{ItemId, Step};

use super::*;

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("imaginer-iced-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch dir");
        Self(dir)
    }

    fn image(&self, name: &str, w: u32, h: u32, exif: Option<Vec<u8>>) -> PathBuf {
        let path = self.0.join(name);
        let pixels = ::image::RgbaImage::from_pixel(w, h, ::image::Rgba([200, 40, 40, 255]));
        let options = EncodeOptions {
            format: Format::from_path(&path).expect("format"),
            quality: 90,
            dpi: None,
            exif,
        };
        codec::write(&pixels, &path, &options).expect("write image");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A minimal big-endian EXIF block with one tag: Make = "Cam".
fn exif_make() -> Vec<u8> {
    let mut b = b"MM\0\x2a\0\0\0\x08".to_vec();
    b.extend_from_slice(&[0, 1]);
    b.extend_from_slice(&[0x01, 0x0F, 0, 2, 0, 0, 0, 4]);
    b.extend_from_slice(b"Cam\0");
    b.extend_from_slice(&[0, 0, 0, 0]);
    b
}

fn app_with(paths: &[&Path]) -> App {
    let (mut app, _) = App::new(paths.iter().map(|p| p.to_path_buf()).collect());
    let _ = app.update(Message::Resized(Size::new(1024.0, 640.0)));
    app
}

/// Clicks a widget by its text and feeds the resulting messages to the app.
fn click(app: &mut App, label: &str) {
    let messages: Vec<Message> = {
        let mut ui = simulator(app.view());
        ui.click(label)
            .unwrap_or_else(|e| panic!("cannot click {label:?}: {e:?}"));
        ui.into_messages().collect()
    };
    for message in messages {
        let _ = app.update(message);
    }
}

fn shows(app: &App, label: &str) -> bool {
    let mut ui = simulator(app.view());
    ui.find(label).is_ok()
}

fn item(app: &App, id: u32) -> imaginer_core::model::Item {
    *app.editor
        .project()
        .state()
        .item(ItemId(id))
        .expect("image")
}

#[test]
fn toolbar_undo_and_redo() {
    // spec: UNDO-001, UNDO-002
    let dir = Scratch::new("undo");
    let a = dir.image("a.png", 40, 30, None);
    let mut app = app_with(&[&a]);
    assert!(app.editor.apply(Step::MoveItem {
        item: ItemId(1),
        x: 5,
        y: 5
    }));
    click(&mut app, "Undo");
    assert_eq!((item(&app, 1).x, item(&app, 1).y), (0, 0));
    click(&mut app, "Redo");
    assert_eq!((item(&app, 1).x, item(&app, 1).y), (5, 5));
}

#[test]
fn undo_tooltip_names_the_step() {
    // spec: UNDO-003
    let dir = Scratch::new("tooltip");
    let a = dir.image("a.png", 40, 30, None);
    let mut app = app_with(&[&a]);
    assert!(app.editor.apply(Step::MoveItem {
        item: ItemId(1),
        x: 5,
        y: 5
    }));
    let _ = app.update(Message::PointerMove(IPoint::new(500.0, 300.0)));
    let mut ui = simulator(app.view());
    ui.point_at(IPoint::new(0.0, 0.0));
    let found = ui.click("Undo").is_ok();
    assert!(found);
    assert_eq!(app.editor.undo_label().as_deref(), Some("Undo Move image"));
}

#[test]
fn context_menu_starts_cropping() {
    // spec: UI-004, CROP-001, CROP-002, CROP-004
    let dir = Scratch::new("menu");
    let a = dir.image("a.png", 400, 300, None);
    let mut app = app_with(&[&a]);
    let _ = app.update(Message::PointerDown(
        IPoint::new(512.0, 320.0),
        PointerButton::Secondary,
    ));
    assert!(shows(&app, "Resize…"));
    click(&mut app, "Crop");
    assert!(app.editor.crop_tool().is_some());
    assert!(shows(&app, "Apply"));
    let _ = app.update(Message::CropField(2, String::new()));
    assert_eq!(app.crop_fields[2], "");
    let _ = app.update(Message::CropField(2, "100".to_owned()));
    click(&mut app, "Apply");
    assert!(app.editor.crop_tool().is_none());
    assert_eq!(item(&app, 1).width, 100);
}

#[test]
fn exif_notice_opens_viewer_and_strips() {
    // spec: META-001, META-002, META-003, META-004
    let dir = Scratch::new("exif");
    let a = dir.image("a.jpg", 40, 30, Some(exif_make()));
    let mut app = app_with(&[&a]);
    click(&mut app, "ⓘ Contains EXIF metadata");
    assert!(matches!(app.modal, Some(Modal::Exif { .. })));
    assert!(shows(&app, "Camera"));
    click(&mut app, "Strip metadata");
    assert!(app.modal.is_none());
    assert!(!shows(&app, "ⓘ Contains EXIF metadata"));
}

#[test]
fn closing_with_changes_asks_to_save() {
    // spec: UI-002
    let dir = Scratch::new("close");
    let a = dir.image("a.png", 40, 30, None);
    let mut app = app_with(&[&a]);
    assert!(app.editor.apply(Step::MoveItem {
        item: ItemId(1),
        x: 5,
        y: 5
    }));
    let _ = app.update(Message::CloseRequested(window::Id::unique()));
    assert!(shows(&app, "Discard"));
    click(&mut app, "Cancel");
    assert!(app.modal.is_none());
}

#[test]
fn opening_several_images_asks_for_the_canvas() {
    // spec: CANVAS-001, UI-001
    let dir = Scratch::new("canvas");
    let a = dir.image("a.png", 100, 50, None);
    let b = dir.image("b.png", 40, 80, None);
    let mut app = app_with(&[]);
    let _ = app.update(Message::Picked(dialogs::Purpose::Open, vec![a, b]));
    assert!(shows(&app, "New composition — canvas size"));
    let _ = app.update(Message::Modal(dialogs::ModalMessage::Choice(1)));
    click(&mut app, "Create");
    assert_eq!(app.editor.project().state().canvas_size(), (100, 130));
}

#[test]
fn exporting_over_a_source_asks_first() {
    // spec: EXPORT-003
    let dir = Scratch::new("export");
    let a = dir.image("a.png", 40, 30, None);
    let mut app = app_with(&[&a]);
    let _ = app.update(Message::Picked(dialogs::Purpose::Export, vec![a.clone()]));
    assert!(shows(&app, "Overwrite"));
    click(&mut app, "Overwrite");
    assert!(shows(&app, "EXIF metadata from"));
    click(&mut app, "Export");
    assert!(app.modal.is_none());
}

#[test]
fn dropping_files_adds_images() {
    // spec: PLACE-001
    let dir = Scratch::new("drop");
    let a = dir.image("a.png", 40, 30, None);
    let b = dir.image("b.png", 20, 20, None);
    let mut app = app_with(&[&a]);
    let c = dir.image("c.png", 20, 20, None);
    let _ = app.update(Message::FileDropped(b));
    let _ = app.update(Message::FileDropped(c));
    let _ = app.update(Message::FlushDrops);
    assert_eq!(app.editor.project().state().items.len(), 3);
}

#[test]
fn snapshot_for_inspection() {
    let Ok(dir) = std::env::var("IMAGINER_SNAPSHOT_DIR") else {
        return;
    };
    let scratch = Scratch::new("snap");
    let a = scratch.image("a.png", 400, 300, None);
    let b = scratch.image("b.png", 300, 300, None);
    let mut app = app_with(&[&a, &b]);
    let _ = app.update(Message::PointerDown(
        IPoint::new(300.0, 560.0),
        PointerButton::Secondary,
    ));
    let mut ui = simulator(app.view());
    let snapshot = ui.snapshot(&Theme::Light).expect("snapshot");
    let path = format!("{dir}/iced-snap.png");
    let _ = std::fs::remove_file(&path);
    snapshot.matches_image(path).expect("write");
}

#[test]
fn edit_menu_names_the_steps() {
    // spec: UNDO-001, UNDO-003
    let dir = Scratch::new("editmenu");
    let a = dir.image("a.png", 40, 30, None);
    let mut app = app_with(&[&a]);
    assert!(app.editor.apply(Step::MoveItem {
        item: ItemId(1),
        x: 5,
        y: 5
    }));
    click(&mut app, "Edit");
    assert!(shows(&app, "Undo Move image  (Ctrl+Z)"));
    click(&mut app, "Undo Move image  (Ctrl+Z)");
    assert_eq!((item(&app, 1).x, item(&app, 1).y), (0, 0));
}
