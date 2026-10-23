# Rust coding standards

Standards for how the Rust code is written. Each standard names what enforces it in an
`Enforced by:` line; `just spec-check` counts that line as the standard's test.

**Production code** is the code built into the application: library and binary targets,
excluding `#[cfg(test)]` modules. **Test code** is everything else (`#[cfg(test)]`
modules, test targets, the cucumber steps).

Clippy runs twice (`just clippy`). The first run checks production code with
`clippy.toml`, which holds every banned macro and method. The second run checks all code,
tests included, with `clippy/tests/clippy.toml`, which holds only the bans that also apply
to test code. A ban for both lives in both files.

To have rust-analyzer show the same diagnostics, set its check command to
`scripts/ra-check.sh`, which runs both passes, in the editor's rust-analyzer settings
(for example `"rust-analyzer.check.overrideCommand": ["scripts/ra-check.sh"]`). A
project-level `rust-analyzer.toml` does not work for this: rust-analyzer 1.99 reads the
file but ignores `check.overrideCommand` from it.

## Panics

### RUST-001: No panic paths
Status: Implemented
Enforced by: clippy (unwrap_used, expect_used, panic, etc), denied in the workspace `Cargo.toml` and `clippy.toml`; review

Production code shall not contain code that can panic (constitution 11). Every failure
path, including one thought to be unreachable, shall be handled by returning an error,
falling back to a safe value, or skipping the work. Test code may panic on purpose.

These clippy lints are denied in the workspace and shall not be removed, lowered, or
suppressed in production code; only test code may allow them
(`decisions/0006-panic-safety.md`):

| Lint | Catches |
|---|---|
| `unwrap_used`, `expect_used`, `get_unwrap`, `unwrap_in_result` | `unwrap`/`expect` on `Option` and `Result` |
| `panic`, `todo`, `unimplemented`, `unreachable` | Explicit panic macros |
| `disallowed_macros` | `assert!`, `assert_eq!`, `assert_ne!` and their `debug_` forms, and `dbg!` (listed in `clippy.toml`) |
| `disallowed_methods` | Standard library methods documented to panic, such as `RefCell::borrow_mut`, `Vec::remove`, `slice::chunks`, `clamp` and integer `pow`/`div_ceil`, plus dependency methods covered by RUST-002 (listed in `clippy.toml`, each with the alternative to use) |
| `panic_in_result_fn` | Panics inside functions that return `Result` |
| `indexing_slicing`, `string_slice` | `a[i]`, `a[i..j]` and `s[i..j]`, which panic when out of range |
| `arithmetic_side_effects` | Integer `+ - * / %` and friends, which panic on overflow or division by zero |
| `unchecked_time_subtraction` | `Instant`/`Duration` subtraction that can underflow |
| `exit` | `std::process::exit`, which skips cleanup |

These can panic but are exceptions to this rule, and are not banned:

- `print!`, `println!`, `eprint!` and `eprintln!`, which panic when the write fails
- allocation failure and running out of memory, including capacity overflow when
  allocating (`Vec::push`, `with_capacity`, `collect`, `format!`, ...)
- `sort` and related methods, which may panic when an `Ord` or comparison is not a total
  order
- `std::thread::scope`, which re-raises a panic from a thread it spawned
- formatting, which panics when a `Display` implementation returns an error

### RUST-002: Panics in dependencies
Status: Implemented
Enforced by: clippy (disallowed_methods, listed in `clippy.toml`); review

Production code shall not call a dependency function in a way that can panic, where the
panic is documented or known from inspecting the dependency's source.

- Where the dependency offers a fallible alternative (for example `image`'s
  `get_pixel_checked` for `get_pixel`), production code shall use it, and the panicking
  function shall be listed in `clippy.toml`'s `disallowed-methods`.
- Where the panic can be ruled out by checking the input first (for example an encoder
  that panics when the buffer length does not match the size), production code shall
  check first and handle the failure.
- Production code may catch a dependency panic with `std::panic::catch_unwind` only when
  the panic is documented and intentional, or known from inspection, and there is no
  panic-safe workaround.
- A documented panic that is unreachable by construction needs neither. The one accepted
  case is glib's `Object::new` and `ObjectBuilder::build`, which panic when the type
  cannot be instantiated or a property does not exist; GTK's generated constructors and
  builders only set properties the type has.

## Errors

### RUST-003: Keep error details
Status: Implemented
Enforced by: clippy (disallowed_methods: `Result::is_ok`, `Result::ok`, `Result::is_err`, in `clippy.toml` and `clippy/tests/clippy.toml`); review

Production and test code shall not call `Result::is_ok`, `Result::ok` or
`Result::is_err`, which throw away the error.

- Production code shall propagate the error, report it, or match it explicitly
  (`match`, `if let`, `let ... else`) where the failure itself is the answer.
- Test code shall keep failure details by using `unwrap()`, `expect()` or
  `expect_err()` instead, not an equivalent such as `matches!(result, Ok(_))`.
- When test code checks that an operation fails, it shall assert the error's details
  (its type or variant, its fields, or its string form), not only that the outcome was
  `Err`.

Clippy's `manual_ok_err` and `redundant_pattern_matching` lints suggest these methods, so
they are allowed in the workspace `Cargo.toml`.

