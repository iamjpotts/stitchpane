# Project file

A project file (`.imaginer`) records a project's sources and the steps applied to them.
It never contains image data. Format choice: `decisions/0002-project-file-format.md`.

### PROJ-001: New project is empty
Status: Implemented

A new project shall have no sources and no steps.

### PROJ-002: Encoding
Status: Implemented

A project file shall be UTF-8 YAML with the extension `.imaginer`.

### PROJ-003: Format version
Status: Implemented

A project file shall have a top-level integer `version`, currently `1`. If a file's
version is newer than the application supports, then opening it shall fail with a
message naming both versions, and the file shall not be changed.

### PROJ-004: Relative source paths
Status: Implemented

Source paths shall be stored relative to the directory containing the project file,
using `/` as the separator, including paths that need `..`. When a project is saved to
a new location, paths shall be rewritten relative to the new location.

### PROJ-005: Missing sources
Status: Implemented

If a source file is missing when a project is opened, then the project shall still
open, each item using that source shall be drawn as a labeled placeholder, and the user
shall be offered to locate the file. Locating it updates the source path.

### PROJ-006: Replay
Status: Implemented

Steps shall be stored in the order they were applied, one entry per step; consecutive
steps are never merged. Opening a project shall replay them in order to produce the same
result as before saving.

### PROJ-008: Redo history
Status: Implemented

Undone steps that can still be redone shall be saved in a `redo` list, ordered so that
the first entry is the next to be redone. Opening a project shall restore them as the
redo history without applying them. Undo history needs no extra storage: it is the
`steps` list.

### PROJ-007: Unknown fields
Status: Implemented

If a project file has a field or step type the application does not recognize, then
opening it shall fail with a message naming it, rather than silently dropping data.

## Example

Illustrative only; field names are settled during implementation.

```yaml
version: 1
canvas: { width: 3000, height: 1200, dpi: 300 }
sources:
  - id: s1
    path: photos/left.heic
  - id: s2
    path: ../shared/right.jpg
steps:
  - add_item: { id: i1, source: s1, x: 0, y: 0 }
  - add_item: { id: i2, source: s2, x: 1400, y: 0 }
  - crop_item: { item: i2, left: 100, top: 0, width: 1600, height: 1200 }
  - add_transition: { id: t1, type: gradient_seam, lower: i1, upper: i2, orientation: horizontal, seam: 1500, width: 200, curve: linear }
  - strip_metadata: { item: i1 }
redo:
  - move_item: { item: i2, x: 1450, y: 0 }
```

## Step types

| Step | Requirement |
|---|---|
| `add_item` | PLACE-001 |
| `move_item` | PLACE-002, UI-006 |
| `resize_item` | PLACE-003, RES-001 |
| `crop_item` | PLACE-004, CROP-004 |
| `set_z_order` | PLACE-005 |
| `rotate_item` | PLACE-007, PLACE-008, UI-006 |
| `set_dpi` | RES-003 |
| `resize_canvas` | CANVAS-002, CANVAS-003 |
| `add_transition`, `edit_transition`, `remove_transition` | BLEND-001 to BLEND-003, BLEND-008 |
| `set_feather` | BLEND-007 |
| `strip_metadata` | META-004 |
| `relink_source` | PROJ-005 |

## Implementation notes

- IDs are numbers. `canvas` is `{ size: fit }` (the canvas follows the images) or
  `{ size: { fixed: { width, height } } }`, plus an optional `dpi`.
