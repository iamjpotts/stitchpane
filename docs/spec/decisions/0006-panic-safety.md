# 0006: Panic safety

Status: Accepted

## Decision

Code that can panic is not acceptable in production code (constitution 11). This is a
permanent decision:

- The panic lints listed in RUST-001 (`engineering/rust.md`) are denied for the whole
  workspace. They shall not be removed or lowered, and production code shall not
  suppress them with `#[allow]` or `#[expect]` at any scope.
- The exceptions listed in RUST-001 are the only panics production code may leave
  unhandled.
- A failure path that is thought to be unreachable is still handled: return an error,
  fall back to a safe value, or skip the work, whichever fits the caller.
- Test code (`#[cfg(test)]` modules and test targets, including the cucumber steps) is
  exempt and may use `unwrap`, `expect`, `assert!` and indexing to keep assertions short.
  It is the only code that may allow the panic lints.

## Consequences

- The lints are set to `deny` in the workspace `Cargo.toml`, so they apply to every
  target. The panicking macros and methods are listed in `clippy.toml`'s
  `disallowed-macros` and `disallowed-methods`.
- `just clippy` runs clippy twice: over production code (`--lib --bins`) with
  `clippy.toml`, then over all targets with `clippy/tests/clippy.toml`
  (`CLIPPY_CONF_DIR`). The second file holds only the bans that apply to test code too
  (RUST-003), so the panicking macros and methods are banned in production code only.
- Test code is exempted from the `Cargo.toml` lints in two ways:
  - `clippy/tests/clippy.toml`'s `allow-*-in-tests` settings exempt `#[test]` functions
    and `#[cfg(test)]` modules from `unwrap_used`, `expect_used`, `panic` and
    `indexing_slicing`; `arithmetic_side_effects` skips them by itself.
  - Each test crate root under `tests/` (today `tests/features.rs`) allows all the
    panic lints, because cucumber steps are not `#[test]` functions.
- `todo`, `unreachable`, `string_slice` and `get_unwrap` have no test exemption, so they
  are banned in `#[cfg(test)]` modules too. That is accepted.
- Review enforces that `#[allow]`/`#[expect]` of a panic lint appears only in test code.
  The lint list is written in `Cargo.toml` and repeated in each test crate's root; some
  drift between the copies is accepted rather than adding a third copy to check them.
- Integer arithmetic in production code uses `checked_`, `saturating_` or `wrapping_`
  operations (or float math) because overflow panics in debug builds.
