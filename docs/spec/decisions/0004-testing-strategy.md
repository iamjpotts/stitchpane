# 0004: Testing strategy

Status: Accepted

## Decision

- **Behavior (middle and top of the pyramid):** cucumber (`cucumber` crate) scenarios in
  `.feature` files, tagged with requirement IDs. Core behavior is tested in
  `crates/imaginer-core/tests/features/`.
- **Units:** ordinary Rust `#[test]` functions for small pieces (geometry, parsing).
- **UI interactions:** almost all interaction logic (hit testing, dragging, snapping,
  keyboard, menus, status text) lives in the core's `Editor`, so the cucumber scenarios
  test it once for both UIs. Each UI crate adds Rust tests for its own wiring:
  - iced: `iced_test`'s headless simulator renders the real view and clicks widgets by
    their text (software renderer, `ICED_TEST_BACKEND=tiny-skia`, set by the Justfile).
    This works well and could host cucumber scenarios later.
  - GTK: an in-process test builds the real window and checks widgets follow the editor.
    It needs a display and is skipped without one. GTK has no headless input simulation,
    so pointer and keyboard wiring in the GTK build is verified by hand.
- **Image output:** compared against reference images only where a property cannot be
  asserted directly (pixel values, dimensions). Kept to a minimum.

## Consequences

`just spec-check` enforces that every `Implemented` requirement has a test.
