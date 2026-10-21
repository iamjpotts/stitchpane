# Composition

Combining images of varying sizes, aspect ratios and resolutions onto one canvas.

## Canvas

### CANVAS-001: Choose canvas size on creation
Status: Draft

When the user creates a composition from two or more images, the system shall ask for
the canvas size with these choices:

- Side by side: widths summed, height of the tallest (default)
- Stacked: heights summed, width of the widest
- Size of the largest image
- A preset: 1920×1080, 3840×2160, A4 at 300 DPI, US Letter at 300 DPI
- Custom: width and height in pixels, with an optional DPI

For the first two choices, the system shall place the images in that arrangement in the
order they were chosen.

### CANVAS-002: Resize canvas by numbers
Status: Draft

The system shall offer a "Canvas size…" menu item that opens a dialog for width and
height in pixels, and DPI.

### CANVAS-003: Resize canvas with handles
Status: Draft

The canvas shall show handles on its edges and corners. Dragging a handle shall resize
the canvas from that side; items shall keep their positions on screen (dragging the left
edge outward adds space on the left).

### CANVAS-004: Background
Status: Draft

Areas not covered by any item shall be transparent. When exported to a format without
transparency, they shall be white.

## Items

### PLACE-001: Add images
Status: Draft

The user shall be able to add images to a composition with a file chooser or by dropping
files from the file manager onto the canvas. New items go on top of the z-order.

### PLACE-002: Move items
Status: Draft

Dragging an item shall move it. Keyboard adjustments follow UI-006.

### PLACE-003: Resize items
Status: Draft

A selected item shall show resize handles. Corner handles keep the aspect ratio unless
Shift is held. Resizing records a resample of that item, never of the source.

### PLACE-004: Crop items
Status: Draft

The crop tool (CROP-001 to CROP-004) shall work on a selected item.

### PLACE-005: Z-order
Status: Draft

An item's context menu shall offer Bring to front, Bring forward, Send backward, and
Send to back.

### PLACE-006: Snapping
Status: Draft

While the user drags an item, a handle, or a seam, the dragged edge or center shall
snap to canvas edges and center lines, and to other items' edges and center lines, when
within 8 screen pixels. A guide-line shall show while snapped. Holding Alt shall disable
snapping for that drag, and a View menu toggle shall turn snapping off entirely.
Keyboard adjustments (UI-006) never snap.

### PLACE-007: Rotate in quarter turns
Status: Draft

An item's context menu shall offer Rotate 90° clockwise, Rotate 90° counterclockwise and
Rotate 180°, which rotate without resampling.

### PLACE-008: Rotate by any angle
Status: Draft

A selected item shall show a rotation handle. Dragging it rotates the item about its
center; holding Shift limits the angle to 15° steps. The context menu shall also offer
"Rotate…" for entering an angle in degrees (to 0.1°). Rotation records a rotate step;
areas the rotated item no longer covers are transparent.

## Blend transitions

### BLEND-006: Transition types
Status: Draft

The system shall offer three kinds of transition, chosen by the user:

- **Gradient seam.** Between two overlapping items: the upper item fades from opaque to
  transparent across a band centered on a seam line, revealing the lower item.
- **Crossfade.** Between two overlapping items: the whole overlapping region blends at
  a set ratio, with no seam line.
- **Feathered edge.** On one item: chosen edges fade to transparent over a set width,
  whatever is beneath. Not tied to another item.

### BLEND-001: Create a transition between two items
Status: Draft

When two items overlap, the user shall be able to create a gradient seam (horizontal or
vertical) or a crossfade between them from either item's context menu. A seam starts at
the center of the overlap with a band width of half the overlap's extent in the blend
direction.

### BLEND-002: Adjust a seam with the mouse
Status: Draft

While either item of a gradient seam is selected, the seam and both band edges shall be
shown as draggable lines. Dragging the seam moves the band; dragging a band edge changes
its width symmetrically.

### BLEND-003: Transition properties
Status: Draft

Right-clicking a transition shall offer its properties and Remove. A transition between
two items may be switched between gradient seam and crossfade. Properties are:

- Gradient seam: orientation (horizontal or vertical), band width in pixels, curve
  (linear or smooth)
- Crossfade: ratio (BLEND-008)
- Feathered edge: edges, width in pixels, curve (BLEND-007)

### BLEND-007: Feathered edges
Status: Draft

An item's context menu shall offer "Feather edges…", where the user chooses any
combination of left, right, top and bottom edges, a width in pixels (default 10% of the
item's shorter side) and a curve (linear or smooth). While the item is selected, each
feathered edge shall show a handle that can be dragged to change the width.

### BLEND-008: Crossfade ratio
Status: Draft

A crossfade shall have a ratio from 0% to 100% (default 50%): the upper item's opacity
within the overlap.

### BLEND-004: Three or more overlapping items
Status: Draft

Rendering composites items bottom to top in z-order. Every transition changes only one
item's transparency: the upper item of a seam or crossfade, or the feathered item
itself. When an item has several transitions, their transparencies multiply. Where three
or more items overlap, each item is composited over the result so far.

### BLEND-005: Transitions follow their items
Status: Draft

When either item moves or resizes, its transitions shall stay attached: a seam keeps its
position relative to the overlap. If the items stop overlapping, the transition is kept
but has no effect until they overlap again.
