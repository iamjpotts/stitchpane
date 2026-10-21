//! Cucumber runner for the `.feature` files under `tests/features`. Scenarios are tagged
//! with the requirement IDs they verify (see `docs/spec/README.md`).

use cucumber::{World, given, then};
use imaginer_core::project::Project;

#[derive(Debug, Default, World)]
struct ProjectWorld {
    project: Option<Project>,
}

impl ProjectWorld {
    fn project(&self) -> &Project {
        self.project
            .as_ref()
            .expect("no project; add a `Given ... project` step")
    }
}

#[given("a new project")]
fn new_project(world: &mut ProjectWorld) {
    world.project = Some(Project::new());
}

#[then(expr = "the project has {int} sources")]
fn source_count(world: &mut ProjectWorld, count: usize) {
    assert_eq!(world.project().sources().len(), count);
}

fn main() {
    futures::executor::block_on(
        ProjectWorld::cucumber()
            .fail_on_skipped()
            .run_and_exit(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/features")),
    );
}
