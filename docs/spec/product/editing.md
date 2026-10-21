# Editing

## Undo and redo

### UNDO-001: Undo
Status: Draft

Every step shall be undoable. Undo shall be available from the Edit menu, a toolbar
button, and Ctrl+Z.

### UNDO-002: Redo
Status: Draft

Undone steps shall be redoable, from the Edit menu, a toolbar button, Ctrl+Shift+Z, and
Ctrl+Y. Recording a new step after an undo shall discard the redo history.

### UNDO-003: Name the action
Status: Draft

The undo and redo menu items and button tooltips shall name the step they will undo or
redo, for example "Undo Move image" or "Redo Crop". When there is nothing to undo or
redo, they shall be disabled.

### UNDO-004: One step per action
Status: Draft

A drag shall produce one step when the mouse button is released, not one per movement.
Typing in a numeric field shall produce one step when the value is committed.

## Open questions

- Should the undo history survive saving and reopening a project? (Currently: the
  project file holds the applied steps, so undo still works after reopening; redo
  history is not saved.)
