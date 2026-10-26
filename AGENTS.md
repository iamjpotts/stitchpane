
# Document organization

* README.md is a one-pager or two pager for humans. Severely limit what it contains to just the essentials
* AGENTS.md is for additional notes not designated for elsewhere
* The specification lives in `docs/spec/`; start at `docs/spec/README.md`. AGENTS.md should not repeat its contents. Project constraints are in `docs/spec/constitution.md`

# Working from the specification

* Change the spec before the code: update or add requirements in `docs/spec/` first, then tests, then code
* If the spec is ambiguous or silent about behavior you need, stop and ask; do not invent behavior
* Tag tests with the requirement IDs they verify (see `docs/spec/README.md`)

# Adding Dependencies

* Add dependencies to the workspace `Cargo.toml` (`[workspace.dependencies]`) and reference them from member crates with `workspace = true`
* After adding them, check with `just deny`
* `cargo deny` sees only Rust crate metadata. When adding a system library (a build or runtime dependency in README.md) or a crate that bundles C code, check its license against constitution item 12 by hand; if it fails, stop and report

# Resolving Cargo Deny Warnings and Errors

* RUSTSEC and CVE errors - revert the Cargo.toml change, stop, report the cargo deny failure you saw, and ask/prompt for how to proceed
* "Stop" means the agent stops all work immediately and reports; it does not continue with other tasks
* New licenses - before adding one to the license list, check it against constitution item 12 (`docs/spec/constitution.md`) for compatibility and obligations. GPL-3.0 and AGPL-3.0 (any variant) are prohibited. If it fails: revert, stop, and report
* Duplicate crates (multiple versions of same crate) - add a `skip` entry for the older version (or a `skip-tree` when a whole platform-only subtree is duplicated) with a one-line comment naming the direct dependency, and the crate within its tree, that requires the older version. Document duplicates for every target, including Windows and macOS
* Every exception in `deny.toml` (advisory ignore, skip, skip-tree) has a one-line comment
* Other errors/warnings - stop, report the change, and ask/prompt for how to proceed

# Verifying the application

Run `just verify`. It runs formatting, `just index-refresh` (so `cargo deny` sees recent yanks), `cargo deny`, checks and clippy (for both the GTK and iced builds, warnings are errors), `just spec-check`, and tests (including cucumber features).

Building needs the `libgtk-4-dev`, `libheif-dev` and `jq` packages (see README.md). The GTK UI test needs a display and skips itself without one.

Before finishing UI work, also look at the result: `just run` (GTK) or `just run iced`.
