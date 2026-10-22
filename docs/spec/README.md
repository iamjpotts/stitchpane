# Specification

This directory is the source of truth for what Imaginer does. Code and tests follow it;
when they disagree, the spec wins until the spec is changed.

## Layout

| Path | Contents |
|---|---|
| `constitution.md` | Principles and constraints every change must respect |
| `glossary.md` | Defined terms; specs use these words with exactly these meanings |
| `non-goals.md` | Things deliberately out of scope |
| `product/` | Functional requirements, by capability |
| `formats/` | File formats (the `.imaginer` project file) |
| `ui/` | Workflows, layout, gestures, shortcuts |
| `architecture.md` | Crates, data flow, where behavior lives |
| `decisions/` | Architecture decision records (ADRs), numbered, never renumbered |
| `work/` | Transient task lists for in-progress work, deleted when done |

## Requirements

Each requirement is a level-3 heading with a stable ID, followed by a status line:

```markdown
### CROP-001: Crop with handles
Status: Draft

When the user drags a crop handle, the system shall ...
```

- **IDs** are `PREFIX-NNN`. Numbers are never reused; a removed requirement keeps its
  heading with `Status: Removed`.
- **Status** is one of `Draft` (proposed, may change freely), `Accepted` (agreed, not yet
  built), `Implemented` (built and tested), `Removed`.
- **Wording** uses "shall" for required behavior and "may" for optional behavior. Prefer
  the patterns *When <trigger>, the system shall <response>*, *While <state>, ...* and
  *If <unwanted condition>, then ...*.
- **Open questions** go in an `## Open questions` section of the file they affect.

### ID prefixes

| Prefix | File |
|---|---|
| `PROJ` | `formats/project-file.md` |
| `CROP`, `RES`, `CONV` | `product/single-image.md` |
| `CANVAS`, `PLACE`, `BLEND` | `product/composition.md` |
| `UNDO` | `product/editing.md` |
| `META` | `product/metadata.md` |
| `EXPORT` | `product/export.md` |
| `SMEAR` | `product/retouch.md` |
| `UI` | `ui/workflows.md` |

## Tests and traceability

Acceptance criteria are written as Given/When/Then scenarios in cucumber `.feature`
files under each crate's `tests/features/`, tagged with the IDs they verify
(`@CROP-001`). Unit tests reference IDs with a `spec: CROP-001` comment where useful.
`just spec-check` (part of `just verify`) fails when an `Implemented` requirement has no
test, or a test names an ID that does not exist. See
`decisions/0004-testing-strategy.md`.

## Workflow

1. Change the spec first (status `Draft`), and resolve its open questions.
2. Mark the requirements `Accepted`.
3. Write the scenarios, then the code, until they pass.
4. Mark the requirements `Implemented`.

Large changes may use a task list in `work/NNN-name/tasks.md`.
