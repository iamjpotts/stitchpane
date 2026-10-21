//! Cucumber runner for the `.feature` files under `tests/features`. Scenarios are tagged
//! with the requirement IDs they verify (see `docs/spec/README.md`).

mod steps;

use cucumber::World as _;
use steps::World;

fn main() {
    futures::executor::block_on(
        World::cucumber()
            .fail_on_skipped()
            .run_and_exit(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/features")),
    );
}
