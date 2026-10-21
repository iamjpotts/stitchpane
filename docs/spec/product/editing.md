# Editing

## Undo and redo

### UNDO-001: Undo
Status: Implemented

Every step shall be undoable. Undo shall be available from the Edit menu, a toolbar
button, and Ctrl+Z.

### UNDO-002: Redo
Status: Implemented

Undone steps shall be redoable, from the Edit menu, a toolbar button, Ctrl+Shift+Z, and
Ctrl+Y. Recording a new step after an undo shall discard the redo history.

### UNDO-003: Name the action
Status: Implemented

The undo and redo menu items and button tooltips shall name the step they will undo or
redo, for example "Undo Move image" or "Redo Crop". When there is nothing to undo or
redo, they shall be disabled.

### UNDO-004: One step per action
Status: Implemented

A drag shall produce one step when the mouse button is released, not one per movement.
Typing in a numeric field shall produce one step when the value is committed. Repeated
arrow-key adjustments of the same target shall produce one step (UI-006).

### UNDO-005: History survives saving
Status: Implemented

Undo and redo history shall be saved in the project file (PROJ-008), so that after
reopening a project the user can undo and redo exactly as before saving.

## Implementation notes

- An unfinished run of key presses counts as an unsaved change and is undone as one step.
