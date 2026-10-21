# Constitution

Principles and constraints that apply to every change. Changing one of these needs an
ADR in `decisions/`.

## Product

1. **Ease of use first.** Prefer direct manipulation (drag handles, right-click menus)
   with numeric entry available as an alternative.
2. **Non-destructive.** Source images are never modified. All edits are steps recorded
   in a project; output is produced by export.

## Platform

3. The application is a **GUI**, not a TUI.
4. The user interface is **native**: GTK 4 (the default build) or iced (an alternative
   build). See `decisions/0001-ui-toolkits.md`.
5. **Ubuntu 24.04 or later** is the only target. Windows and macOS are not targets.
6. Library and binary size are not concerns.

## Implementation

7. **No web stack.** JavaScript, TypeScript, npm, pnpm, Electron, webviews and related
   technologies are prohibited because of supply-chain risk.
8. **Rust preferred.** C libraries are acceptable through Rust bindings where needed
   (GTK, image codecs).
9. **Dependencies pass `cargo deny`.** Every exception in `deny.toml` carries a one-line
   comment explaining it.
10. **No GPL-3.0 or AGPL-3.0 crates**, in any variant.

## Licensing

11. Imaginer is currently unlicensed (private, personal use). Dependency license
    obligations will be reviewed when a license is chosen.
