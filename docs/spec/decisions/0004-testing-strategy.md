# 0004: Testing strategy

Status: Accepted

## Decision

- **Behavior (middle and top of the pyramid):** cucumber (`cucumber` crate) scenarios in
  `.feature` files, tagged with requirement IDs. Core behavior is tested in
  `crates/imaginer-core/tests/features/`.
- **Units:** ordinary Rust `#[test]` functions for small pieces (geometry, parsing).
- **UI interactions:** cucumber scenarios per UI crate, driving the UI in-process. For
  iced, `iced_test` provides a headless simulator that can click and type. For GTK,
  tests need a display (or a headless compositor). To be proven on the first UI feature.
- **Image output:** compared against reference images only where a property cannot be
  asserted directly (pixel values, dimensions). Kept to a minimum.

## Consequences

`just spec-check` enforces that every `Implemented` requirement has a test.
