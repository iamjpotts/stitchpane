use cucumber::gherkin::Step as GherkinStep;
use cucumber::{given, then, when};
use stitchpane_core::editor::{CanvasChoice, Editor, Preset};
use stitchpane_core::model::{ItemId, SourceId, Step};
use stitchpane_core::render::SourceProblem;

use super::{World, ensure_parent};

fn yaml(world: &World, file: &str) -> yaml_serde::Value {
    let text = std::fs::read_to_string(world.path(file)).expect("read project file");
    yaml_serde::from_str(&text).expect("valid YAML")
}

#[given("a new project")]
fn new_project(world: &mut World) {
    world.editor = Editor::new();
    world.editor.set_viewport(1000.0, 800.0);
}

#[given(expr = "an image {string} of {int} by {int} pixels added to the project")]
fn add_image(world: &mut World, file: String, width: u32, height: u32) {
    let path = world.path(&file);
    super::files::write_solid(world, &path, width, height, [200, 100, 50, 255]);
    world.editor.add_images(&[path], None).expect("add image");
    world.item = world.editor.project().state().items.last().map(|i| i.id);
}

#[given(expr = "the image is moved to {int}, {int}")]
#[when(expr = "the image is moved to {int}, {int}")]
fn move_image(world: &mut World, x: i32, y: i32) {
    let item = world.item();
    assert!(world.editor.apply(Step::MoveItem { item, x, y }));
}

#[given(expr = "image {int} is moved to {int}, {int}")]
#[when(expr = "image {int} is moved to {int}, {int}")]
fn move_item(world: &mut World, id: u32, x: i32, y: i32) {
    assert!(world.editor.apply(Step::MoveItem {
        item: ItemId(id),
        x,
        y
    }));
}

#[given(expr = "a project file {string} containing:")]
fn project_file_containing(world: &mut World, file: String, step: &GherkinStep) {
    let path = world.path(&file);
    ensure_parent(&path);
    let text = step.docstring.as_ref().expect("docstring").trim_start();
    std::fs::write(&path, text).expect("write project file");
    world.snapshots.push((path, text.as_bytes().to_vec()));
}

#[given(expr = "I save the project as {string}")]
#[when(expr = "I save the project as {string}")]
fn save_as(world: &mut World, file: String) {
    let path = world.path(&file);
    ensure_parent(&path);
    world.editor.save_as(&path).expect("save project");
}

#[given(expr = "I open {string}")]
#[when(expr = "I open {string}")]
fn open(world: &mut World, file: String) {
    match world.editor.open(&world.path(&file)) {
        Ok(()) => {
            world.error = None;
            world.item = world.editor.project().state().items.last().map(|i| i.id);
        }
        Err(e) => world.error = Some(e),
    }
}

fn open_together(world: &mut World, files: &[String]) {
    let paths: Vec<_> = files.iter().map(|f| world.path(f)).collect();
    world.editor.open_images(&paths).expect("open images");
}

#[when(expr = "I open {string} and {string} together")]
fn open_two(world: &mut World, a: String, b: String) {
    open_together(world, &[a, b]);
}

#[when(expr = "I open {string}, {string} and {string} together")]
fn open_three(world: &mut World, a: String, b: String, c: String) {
    open_together(world, &[a, b, c]);
}

fn choice_named(name: &str) -> CanvasChoice {
    let fixed = [
        CanvasChoice::SideBySide,
        CanvasChoice::Stacked,
        CanvasChoice::Largest,
    ];
    fixed
        .into_iter()
        .chain(Preset::ALL.map(CanvasChoice::Preset))
        .find(|c| c.label() == name)
        .unwrap_or_else(|| panic!("no canvas choice {name}"))
}

fn compose(world: &mut World, files: &[&str], choice: CanvasChoice) {
    let paths: Vec<_> = files.iter().map(|f| world.path(f)).collect();
    world
        .editor
        .new_composition(&paths, Some(choice))
        .expect("create composition");
}

#[given(expr = "a composition of {string} and {string} side by side")]
fn composition_side_by_side(world: &mut World, a: String, b: String) {
    compose(world, &[&a, &b], CanvasChoice::SideBySide);
}

#[given(expr = "a composition of {string} and {string} with canvas {string}")]
#[when(expr = "I create a composition of {string} and {string} with canvas {string}")]
fn composition_with(world: &mut World, a: String, b: String, choice: String) {
    compose(world, &[&a, &b], choice_named(&choice));
}

#[given(
    regex = r#"^a composition of "([^"]+)" and "([^"]+)" with a custom canvas of (\d+) by (\d+) pixels(?: at (\d+) DPI)?$"#
)]
#[when(
    regex = r#"^I create a composition of "([^"]+)" and "([^"]+)" with a custom canvas of (\d+) by (\d+) pixels(?: at (\d+) DPI)?$"#
)]
fn composition_custom(world: &mut World, a: String, b: String, w: u32, h: u32, dpi: String) {
    let dpi = (!dpi.is_empty()).then(|| dpi.parse::<f64>().expect("DPI is a number"));
    compose(
        world,
        &[&a, &b],
        CanvasChoice::Custom {
            width: w,
            height: h,
            dpi,
        },
    );
}

#[given(expr = "a composition of {string} with a custom canvas of {int} by {int} pixels")]
fn composition_single(world: &mut World, a: String, w: u32, h: u32) {
    compose(
        world,
        &[&a],
        CanvasChoice::Custom {
            width: w,
            height: h,
            dpi: None,
        },
    );
}

#[when(expr = "I add {string}")]
fn add(world: &mut World, file: String) {
    let path = world.path(&file);
    world.editor.add_images(&[path], None).expect("add image");
}

#[when(expr = "I drop {string} on the canvas at {int}, {int}")]
fn drop_at(world: &mut World, file: String, x: f64, y: f64) {
    let path = world.path(&file);
    let at = world.screen(x, y);
    world
        .editor
        .add_images(&[path], Some(at))
        .expect("drop image");
}

#[when(expr = "the file {string} is deleted")]
fn delete_file(world: &mut World, file: String) {
    std::fs::remove_file(world.path(&file)).expect("delete file");
}

#[when(expr = "I locate source {int} at {string}")]
fn locate(world: &mut World, id: u32, file: String) {
    let path = world.path(&file);
    assert!(world.editor.relink(SourceId(id), &path));
}

#[when("I undo")]
fn undo(world: &mut World) {
    assert!(world.editor.undo(), "nothing to undo");
}

#[when("I redo")]
fn redo(world: &mut World) {
    assert!(world.editor.redo(), "nothing to redo");
}

#[then(expr = "opening fails with a message containing {string}")]
#[then(expr = "the message contains {string}")]
fn error_contains(world: &mut World, text: String) {
    let error = world.error.as_deref().expect("opening should have failed");
    assert!(error.contains(&text), "{error:?} does not contain {text:?}");
}

#[then(expr = "{string} is unchanged")]
#[then(expr = "{string} is unchanged on disk")]
fn unchanged(world: &mut World, file: String) {
    let path = world.path(&file);
    let (_, before) = world
        .snapshots
        .iter()
        .find(|(p, _)| *p == path)
        .expect("snapshot");
    assert_eq!(&std::fs::read(&path).expect("read"), before);
}

#[then(expr = "{string} is a YAML document")]
fn is_yaml(world: &mut World, file: String) {
    assert!(file.ends_with(".stitchpane"));
    let value = yaml(world, &file);
    assert_eq!(value["version"].as_u64(), Some(1));
}

#[then(expr = "{string} lists the source path {string}")]
fn lists_source(world: &mut World, file: String, path: String) {
    let value = yaml(world, &file);
    let paths: Vec<&str> = value["sources"]
        .as_sequence()
        .expect("sources")
        .iter()
        .filter_map(|s| s["path"].as_str())
        .collect();
    assert!(paths.contains(&path.as_str()), "{paths:?}");
}

#[then(expr = "{string} has {int} steps")]
fn file_steps(world: &mut World, file: String, count: usize) {
    let value = yaml(world, &file);
    assert_eq!(value["steps"].as_sequence().expect("steps").len(), count);
}

#[then(expr = "the project's source {int} is the file {string}")]
fn source_is(world: &mut World, id: u32, file: String) {
    let source = world.editor.project().source(SourceId(id)).expect("source");
    assert_eq!(source.path, world.path(&file));
}

#[then(expr = "the project has {int} sources")]
fn source_count(world: &mut World, count: usize) {
    assert_eq!(world.editor.project().sources().len(), count);
}

#[then(expr = "the project has {int} steps")]
fn step_count(world: &mut World, count: usize) {
    assert_eq!(world.editor.project().steps().len(), count);
}

#[then(expr = "the project has {int} images")]
fn image_count(world: &mut World, count: usize) {
    assert_eq!(world.editor.display_state().items.len(), count);
}

#[then(expr = "source {int} is missing")]
fn source_missing(world: &mut World, id: u32) {
    let missing = world.editor.missing_sources();
    assert!(
        missing
            .iter()
            .any(|(s, _, p)| *s == SourceId(id) && *p == SourceProblem::Missing),
        "{missing:?}"
    );
}

#[then("no sources are missing")]
fn none_missing(world: &mut World) {
    assert!(world.editor.missing_sources().is_empty());
}

#[then(expr = "image {int} is labeled {string}")]
fn labeled(world: &mut World, _id: u32, text: String) {
    let labels = world.editor.labels();
    assert!(labels.iter().any(|(_, t)| *t == text), "{labels:?}");
}

#[then(expr = "the undo label is {string}")]
fn undo_label(world: &mut World, label: String) {
    assert_eq!(world.editor.undo_label().as_deref(), Some(label.as_str()));
}

#[then(expr = "the redo label is {string}")]
fn redo_label(world: &mut World, label: String) {
    assert_eq!(world.editor.redo_label().as_deref(), Some(label.as_str()));
}

#[then("there is nothing to undo")]
fn nothing_to_undo(world: &mut World) {
    assert_eq!(world.editor.undo_label(), None);
}

#[then("there is nothing to redo")]
fn nothing_to_redo(world: &mut World) {
    assert_eq!(world.editor.redo_label(), None);
}

#[then(expr = "the title is {string}")]
fn title_is(world: &mut World, title: String) {
    assert_eq!(world.editor.title(), title);
}

#[then("there are unsaved changes")]
fn unsaved(world: &mut World) {
    assert!(world.editor.needs_save_prompt());
}

#[then("there are no unsaved changes")]
fn saved(world: &mut World) {
    assert!(!world.editor.needs_save_prompt());
}

#[when(expr = "I pick {string} in a file dialog")]
fn pick_in_dialog(world: &mut World, file: String) {
    let path = world.path(&file);
    world.editor.remember_folder(&path);
}

#[then("file dialogs start in the default folder")]
fn dialogs_default(world: &mut World) {
    assert_eq!(world.editor.dialog_folder(), None);
}

#[then(expr = "file dialogs start in the folder of {string}")]
fn dialogs_start_in(world: &mut World, file: String) {
    let path = world.path(&file);
    assert_eq!(world.editor.dialog_folder(), path.parent());
}
