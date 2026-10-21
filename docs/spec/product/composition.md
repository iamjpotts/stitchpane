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

Dragging an item shall move it. Arrow keys shall nudge the selected item by 1 pixel, or
10 pixels with Shift.

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

## Blend transitions

What a "blend transition" means is not settled. Option A is the working assumption;
options B and C are alternatives kept until the user chooses.

- **A. Gradient seam (default).** Where two items overlap, the upper item fades from
  opaque to transparent across a band centered on a seam line, revealing the lower item.
- **B. Feathered edge.** An item's chosen edges fade to transparent over a set width,
  whatever is beneath it. Not tied to a pair of items.
- **C. Full-overlap crossfade.** The entire overlapping region blends evenly (50/50 or a
  set ratio), with no seam line.

### BLEND-001: Create a transition
Status: Draft

When two items overlap, the user shall be able to create a horizontal or vertical
transition between them from either item's context menu. The seam starts at the center
of the overlap and the band width starts at half the overlap's extent in the blend
direction.

### BLEND-002: Adjust with the mouse
Status: Draft

The seam and both band edges shall be shown as draggable lines while either item is
selected. Dragging the seam moves the band; dragging a band edge changes its width
symmetrically.

### BLEND-003: Transition properties
Status: Draft

Right-clicking a transition shall offer: orientation (horizontal or vertical), band
width in pixels, curve (linear or smooth), and Remove.

### BLEND-004: Three or more overlapping items
Status: Draft

Transitions are always between exactly two items. Rendering composites items bottom to
top in z-order; a transition changes only the upper item's transparency inside its band.
Where three or more items overlap, each upper item's transitions are applied in turn as
it is composited over the result so far.

### BLEND-005: Transitions follow their items
Status: Draft

When either item moves or resizes, its transitions shall stay attached: the seam keeps
its position relative to the overlap. If the items stop overlapping, the transition is
kept but has no effect until they overlap again.

## Open questions

- Which of transition options A, B and C is intended (several may be wanted)?
- Should items snap to canvas edges and to each other while dragging?
- Should rotation of items be supported?
