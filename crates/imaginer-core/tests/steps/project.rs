use cucumber::gherkin::Step as GherkinStep;
use cucumber::{given, then, when};
use imaginer_core::model::{ItemId, SourceId, Step};
use imaginer_core::project::Project;
use imaginer_core::project_file;
use imaginer_core::render::{Renderer, SourceProblem};

use super::{World, ensure_parent};

fn yaml(world: &World, file: &str) -> yaml_serde::Value {
    let text = std::fs::read_to_string(world.path(file)).expect("read project file");
    yaml_serde::from_str(&text).expect("valid YAML")
}

#[given("a new project")]
fn new_project(world: &mut World) {
    world.project = Project::new();
}

#[given(expr = "an image {string} of {int} by {int} pixels added to the project")]
fn add_image(world: &mut World, file: String, width: u32, height: u32) {
    let path = world.path(&file);
    ensure_parent(&path);
    image::RgbaImage::from_pixel(width, height, image::Rgba([200, 100, 50, 255]))
        .save(&path)
        .expect("write test image");
    let source = world.project.add_source(path);
    let id = world.project.state().next_item_id();
    world
        .project
        .apply(Step::AddItem {
            id,
            source,
            x: 0,
            y: 0,
            source_width: width,
            source_height: height,
        })
        .expect("add image");
    world.item = Some(id);
}

#[given(expr = "the image is moved to {int}, {int}")]
#[when(expr = "the image is moved to {int}, {int}")]
fn move_image(world: &mut World, x: i32, y: i32) {
    let item = world.item();
    world
        .project
        .apply(Step::MoveItem { item, x, y })
        .expect("move image");
}

#[given(expr = "a project file {string} containing:")]
fn project_file_containing(world: &mut World, file: String, step: &GherkinStep) {
    let path = world.path(&file);
    ensure_parent(&path);
    let text = step.docstring.as_ref().expect("docstring").trim_start();
    std::fs::write(&path, text).expect("write project file");
    world.snapshots.push((path, text.as_bytes().to_vec()));
}

#[when(expr = "I save the project as {string}")]
fn save_as(world: &mut World, file: String) {
    let path = world.path(&file);
    ensure_parent(&path);
    project_file::save(&mut world.project, &path).expect("save project");
}

#[when(expr = "I open {string}")]
fn open(world: &mut World, file: String) {
    match project_file::load(&world.path(&file)) {
        Ok(project) => {
            world.project = project;
            world.error = None;
        }
        Err(e) => world.error = Some(e.to_string()),
    }
}

#[when(expr = "the file {string} is deleted")]
fn delete_file(world: &mut World, file: String) {
    std::fs::remove_file(world.path(&file)).expect("delete file");
}

#[when("I undo")]
fn undo(world: &mut World) {
    assert!(world.project.undo(), "nothing to undo");
}

#[when("I redo")]
fn redo(world: &mut World) {
    assert!(world.project.redo(), "nothing to redo");
}

#[then(expr = "opening fails with a message containing {string}")]
#[then(expr = "the message contains {string}")]
fn error_contains(world: &mut World, text: String) {
    let error = world.error.as_deref().expect("opening should have failed");
    assert!(error.contains(&text), "{error:?} does not contain {text:?}");
}

#[then(expr = "{string} is unchanged")]
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
    assert!(file.ends_with(".imaginer"));
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
    let source = world.project.source(SourceId(id)).expect("source");
    assert_eq!(source.path, world.path(&file));
}

#[then(expr = "the project has {int} sources")]
fn source_count(world: &mut World, count: usize) {
    assert_eq!(world.project.sources().len(), count);
}

#[then(expr = "the project has {int} steps")]
fn step_count(world: &mut World, count: usize) {
    assert_eq!(world.project.steps().len(), count);
}

#[then(expr = "source {int} is missing")]
fn source_missing(world: &mut World, id: u32) {
    let problems = Renderer::new().problems(world.project.state());
    assert!(
        problems.contains(&(SourceId(id), SourceProblem::Missing)),
        "{problems:?}"
    );
}

#[then(expr = "image {int} is at {int}, {int}")]
fn image_at(world: &mut World, id: u32, x: i32, y: i32) {
    let item = world.project.state().item(ItemId(id)).expect("image");
    assert_eq!((item.x, item.y), (x, y));
}

#[then(expr = "the undo label is {string}")]
fn undo_label(world: &mut World, label: String) {
    assert_eq!(world.project.undo_label().as_deref(), Some(label.as_str()));
}

#[then(expr = "the redo label is {string}")]
fn redo_label(world: &mut World, label: String) {
    assert_eq!(world.project.redo_label().as_deref(), Some(label.as_str()));
}

#[then("there is nothing to undo")]
fn nothing_to_undo(world: &mut World) {
    assert_eq!(world.project.undo_label(), None);
}

#[then("there is nothing to redo")]
fn nothing_to_redo(world: &mut World) {
    assert_eq!(world.project.redo_label(), None);
}
