# 0005: Workspace layout

Status: Accepted

## Decision

A Cargo workspace with four crates: `stitchpane-core`, `stitchpane-gtk`,
`stitchpane-iced`, and the `stitchpane` binary, which selects one UI crate by feature
(`gtk` default, `iced` alternative; mutually exclusive). Licenses influenced this: each
toolkit's dependency tree, and its license obligations, stays in its own crate.

## Consequences

- Lints, tests and checks run across the workspace (`just verify`), with the iced build
  checked separately because the features are mutually exclusive.
