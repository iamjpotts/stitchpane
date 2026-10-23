//! Cucumber runner for the `.feature` files under `tests/features`. Scenarios are tagged
//! with the requirement IDs they verify (see `docs/spec/README.md`).

// test code may panic to keep its assertions short (RUST-001); allows every panic lint
// set in the workspace Cargo.toml, and the macro and method bans in clippy.toml
#![allow(
    clippy::arithmetic_side_effects,
    clippy::disallowed_macros,
    clippy::disallowed_methods,
    clippy::exit,
    clippy::expect_used,
    clippy::get_unwrap,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::panic_in_result_fn,
    clippy::string_slice,
    clippy::todo,
    clippy::unchecked_time_subtraction,
    clippy::unimplemented,
    clippy::unreachable,
    clippy::unwrap_in_result,
    clippy::unwrap_used
)]

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
