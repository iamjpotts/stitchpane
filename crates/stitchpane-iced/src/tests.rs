//! UI tests driven through the headless iced simulator. Each test names the requirement it
//! verifies in a `spec:` comment.

use std::path::{Path, PathBuf};

use iced::{Size, Theme};
use iced_test::simulator;
use stitchpane_core::codec::{self, EncodeOptions, Format};
use stitchpane_core::model::{ItemId, Step};

use super::*;

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("stitchpane-iced-{name}-{}", std::process::id()));
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

/// Delivers the allocations the headless tests never run, as iced would (UI-011).
fn present(app: &mut App) {
    while let Some(id) = app.frames.uploading.as_ref().map(|f| f.handle.id()) {
        let _ = app.update(Message::FrameReady(id, None));
    }
}

fn shown_id(app: &App) -> Option<ImageId> {
    app.frames.shown().map(|f| f.handle.id())
}

fn uploading_id(app: &App) -> Option<ImageId> {
    app.frames.uploading.as_ref().map(|f| f.handle.id())
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

fn assert_shows(app: &App, label: &str) {
    let mut ui = simulator(app.view());
    if let Err(error) = ui.find(label) {
        panic!("{label:?} is not shown: {error}");
    }
}

fn assert_hides(app: &App, label: &str) {
    let mut ui = simulator(app.view());
    let error = ui.find(label).map(|_| ()).expect_err("label is shown");
    assert!(
        matches!(&error, iced_test::Error::SelectorNotFound { selector } if selector.contains(label)),
        "{error}"
    );
}

fn item(app: &App, id: u32) -> stitchpane_core::model::Item {
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
    if let Err(error) = ui.click("Undo") {
        panic!("Undo is not clickable: {error}");
    }
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
    assert_shows(&app, "Resize…");
    click(&mut app, "Crop");
    assert!(app.editor.crop_tool().is_some());
    assert_shows(&app, "Apply");
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
    assert_shows(&app, "Camera");
    click(&mut app, "Strip metadata");
    assert!(app.modal.is_none());
    assert_hides(&app, "ⓘ Contains EXIF metadata");
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
    assert_shows(&app, "Discard");
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
    assert_shows(&app, "New composition — canvas size");
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
    assert_shows(&app, "Overwrite");
    click(&mut app, "Overwrite");
    assert_shows(&app, "EXIF metadata from");
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
    let Ok(dir) = std::env::var("STITCHPANE_SNAPSHOT_DIR") else {
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
    present(&mut app);
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
    assert_shows(&app, "Undo Move image  (Ctrl+Z)");
    click(&mut app, "Undo Move image  (Ctrl+Z)");
    assert_eq!((item(&app, 1).x, item(&app, 1).y), (0, 0));
}

#[test]
fn save_button_saves() {
    // spec: UI-008
    let dir = Scratch::new("save");
    let a = dir.image("a.png", 40, 30, None);
    let mut app = app_with(&[&a]);
    app.editor
        .save_as(&dir.0.join("p.stitchpane"))
        .expect("save");
    assert!(app.editor.apply(Step::MoveItem {
        item: ItemId(1),
        x: 5,
        y: 5
    }));
    click(&mut app, "Save");
    assert!(!app.editor.is_dirty());
}

#[test]
fn file_dialogs_remember_the_folder() {
    // spec: UI-007
    let dir = Scratch::new("folder");
    let a = dir.image("a.png", 40, 30, None);
    std::fs::create_dir_all(dir.0.join("sub")).expect("sub dir");
    let b = dir.image("sub/b.png", 20, 20, None);
    let mut app = app_with(&[&a]);
    assert_eq!(app.editor.dialog_folder(), Some(dir.0.as_path()));
    let _ = app.update(Message::Picked(dialogs::Purpose::AddImages, vec![b]));
    assert_eq!(
        app.editor.dialog_folder(),
        Some(dir.0.join("sub").as_path())
    );
}

#[test]
fn resampling_happens_in_the_background() {
    // spec: UI-009
    let dir = Scratch::new("background");
    let a = dir.image("a.png", 400, 300, None);
    let mut app = app_with(&[&a]);
    assert!(app.editor.apply(Step::ResizeItem {
        item: ItemId(1),
        x: 0,
        y: 0,
        width: 200,
        height: 150,
        filter: stitchpane_core::model::Filter::Lanczos3,
    }));
    let _ = app.update(Message::Resized(Size::new(1024.0, 640.0)));
    assert!(app.editor.background_busy());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while app.editor.background_busy() {
        assert!(std::time::Instant::now() < deadline);
        let _ = app.update(Message::PollBackground);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[test]
fn status_bar_zoom_opens_the_zoom_menu() {
    // spec: UI-010
    let dir = Scratch::new("zoom");
    let a = dir.image("a.png", 400, 300, None);
    let mut app = app_with(&[&a]);
    let label = app.editor.zoom_text();
    click(&mut app, &label);
    assert_shows(&app, "Zoom in");
    click(&mut app, "100%");
    assert_eq!(app.editor.zoom_text(), "zoom 100%");
}

#[test]
fn smear_bar_sets_the_brush_and_strokes_record_steps() {
    // spec: SMEAR-001, SMEAR-003, SMEAR-004
    let dir = Scratch::new("smear");
    let a = dir.image("a.png", 400, 300, None);
    let mut app = app_with(&[&a]);
    let _ = app.update(Message::PointerDown(
        IPoint::new(512.0, 320.0),
        PointerButton::Secondary,
    ));
    click(&mut app, "Smear");
    assert_shows(&app, "Done");
    let _ = app.update(Message::SmearField(0, "40".to_owned()));
    let _ = app.update(Message::SmearChoice("Blur".to_owned()));
    let tool = app.editor.smear_tool().expect("smear tool");
    assert_eq!((tool.size, tool.kind.label()), (40.0, "Blur"));
    let _ = app.update(Message::PointerDown(
        IPoint::new(500.0, 320.0),
        PointerButton::Primary,
    ));
    let _ = app.update(Message::PointerMove(IPoint::new(540.0, 320.0)));
    let _ = app.update(Message::PointerUp(IPoint::new(560.0, 320.0)));
    assert_eq!(app.editor.undo_label().as_deref(), Some("Undo Smear"));
    click(&mut app, "Done");
    assert!(app.editor.smear_tool().is_none());
}

#[test]
fn delete_removes_the_selected_image() {
    // spec: PLACE-009
    let dir = Scratch::new("remove");
    let a = dir.image("a.png", 40, 30, None);
    let b = dir.image("b.png", 20, 20, None);
    let mut app = app_with(&[&a]);
    let _ = app.update(Message::Picked(dialogs::Purpose::AddImages, vec![b]));
    assert_eq!(app.editor.project().state().items.len(), 2);
    let _ = app.update(Message::Key(
        keyboard::Key::Named(Named::Delete),
        keyboard::Modifiers::default(),
    ));
    assert_eq!(app.editor.project().state().items.len(), 1);
}

#[test]
fn canvas_keeps_the_shown_frame_until_the_next_is_ready() {
    // spec: UI-011
    let dir = Scratch::new("frames");
    let a = dir.image("a.png", 400, 300, None);
    let mut app = app_with(&[&a]);
    present(&mut app);
    let first = shown_id(&app).expect("a frame is shown");

    // a new frame uploads while the old one stays on the canvas
    let _ = app.update(Message::PointerMove(IPoint::new(500.0, 300.0)));
    let second = uploading_id(&app).expect("a frame is uploading");
    assert_eq!(shown_id(&app), Some(first));

    // frames rendered meanwhile wait; only the newest is kept
    let _ = app.update(Message::PointerMove(IPoint::new(510.0, 300.0)));
    let _ = app.update(Message::PointerMove(IPoint::new(520.0, 300.0)));
    let newest = app.frames.next.as_ref().expect("a frame waits").handle.id();
    assert_eq!(uploading_id(&app), Some(second));

    // a stale report changes nothing
    let _ = app.update(Message::FrameReady(first, None));
    assert_eq!(
        (shown_id(&app), uploading_id(&app)),
        (Some(first), Some(second))
    );

    let _ = app.update(Message::FrameReady(second, None));
    assert_eq!(shown_id(&app), Some(second));
    assert_eq!(uploading_id(&app), Some(newest));
    assert!(app.frames.next.is_none());

    let _ = app.update(Message::FrameReady(newest, None));
    assert_eq!(shown_id(&app), Some(newest));
    assert!(app.frames.uploading.is_none());
}
