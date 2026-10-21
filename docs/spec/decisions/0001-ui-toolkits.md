# 0001: UI toolkits

Status: Accepted

## Context

The UI must be native, with no web stack (constitution 4, 7). "Native" can mean
platform-native widgets (GTK on Ubuntu) or a Rust-native toolkit that draws its own
widgets.

## Decision

Support both: GTK 4 through `gtk4` (default) and iced. Each lives in its own crate;
the `imaginer` binary picks one at compile time with the `gtk` or `iced` feature.

## Consequences

- Each workflow is implemented twice; shared behavior must live in `imaginer-core`.
- GTK needs `libgtk-4-dev` to build.
- iced 0.14 brings a large dependency tree, including crates with unmaintained
  advisories, ignored with comments in `deny.toml`.
