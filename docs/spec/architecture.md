# Architecture

## Crates

```
crates/
  imaginer-core   model, steps, rendering, undo history; no UI toolkit
  imaginer-gtk    GTK 4 UI            -> depends on imaginer-core
  imaginer-iced   iced UI             -> depends on imaginer-core
  imaginer        the binary; feature `gtk` (default) or `iced` selects the UI crate
```

The split keeps each toolkit's dependency tree (and its licenses) in its own crate, so a
build contains only the toolkit it uses. See `decisions/0005-workspace-layout.md`.

## Data flow

1. The UI turns a completed user action into a **step** and submits it to the core.
2. The core validates the step, applies it to the project, and pushes it onto the undo
   history.
3. The UI asks the core to render the canvas (or a region of it) and displays the result.

All behavior that both UIs share (validation, geometry, transitions, undo) lives in
`imaginer-core`. UI crates contain only presentation and input handling.

## Rendering

Rendering replays the steps over the sources to produce the canvas image. Decoded
sources and intermediate results may be cached; caching must not change the output.
