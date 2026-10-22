# Composition

Combining images of varying sizes, aspect ratios and resolutions onto one canvas.

## Canvas

### CANVAS-001: Choose canvas size on creation
Status: Implemented

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
Status: Implemented

The system shall offer a "Canvas size…" menu item that opens a dialog for width and
height in pixels, and DPI.

### CANVAS-003: Resize canvas with handles
Status: Implemented

The canvas shall show handles on its edges and corners. Dragging a handle shall resize
the canvas from that side; items shall keep their positions on screen (dragging the left
edge outward adds space on the left).

### CANVAS-004: Background
Status: Implemented

Areas not covered by any item shall be transparent. When exported to a format without
transparency, they shall be white.

## Items

### PLACE-001: Add images
Status: Implemented

The user shall be able to add images to a composition with a file chooser or by dropping
files from the file manager onto the canvas. New items go on top of the z-order.

### PLACE-002: Move items
Status: Implemented

Dragging an item shall move it. Keyboard adjustments follow UI-006.

### PLACE-003: Resize items
Status: Implemented

A selected item shall show resize handles. Corner handles keep the aspect ratio unless
Shift is held. Resizing records a resample of that item, never of the source.

### PLACE-004: Crop items
Status: Implemented

The crop tool (CROP-001 to CROP-005) shall work on a selected item.

### PLACE-005: Z-order
Status: Implemented

An item's context menu shall offer Bring to front, Bring forward, Send backward, and
Send to back.

### PLACE-006: Snapping
Status: Implemented

While the user drags an item, a handle, or a seam, the dragged edge or center shall
snap to canvas edges and center lines, and to other items' edges and center lines, when
within 8 screen pixels. A guide-line shall show while snapped. Holding Alt shall disable
snapping for that drag, and a View menu toggle shall turn snapping off entirely.
Keyboard adjustments (UI-006) never snap.

### PLACE-007: Rotate in quarter turns
Status: Implemented

An item's context menu shall offer Rotate 90° clockwise, Rotate 90° counterclockwise and
Rotate 180°, which rotate without resampling.

### PLACE-008: Rotate by any angle
Status: Implemented

A selected item shall show a rotation handle. Dragging it rotates the item about its
center; holding Shift limits the angle to 15° steps. The context menu shall also offer
"Rotate…" for entering an angle in degrees (to 0.1°). Rotation records a rotate step;
areas the rotated item no longer covers are transparent.

## Blend transitions

### BLEND-006: Transition types
Status: Implemented

The system shall offer three kinds of transition, chosen by the user:

- **Gradient seam.** Between two overlapping items: the upper item fades from opaque to
  transparent across a band centered on a seam line, revealing the lower item.
- **Crossfade.** Between two overlapping items: the whole overlapping region blends at
  a set ratio, with no seam line.
- **Feathered edge.** On one item: chosen edges fade to transparent over a set width,
  whatever is beneath. Not tied to another item.

### BLEND-001: Create a transition between two items
Status: Implemented

When two items overlap, the user shall be able to create a gradient seam (horizontal or
vertical) or a crossfade between them from either item's context menu. A seam starts at
the center of the overlap with a band width of half the overlap's extent in the blend
direction.

### BLEND-002: Adjust a seam with the mouse
Status: Implemented

While either item of a gradient seam is selected, the seam and both band edges shall be
shown as draggable lines. Dragging the seam moves the band; dragging a band edge changes
its width symmetrically.

### BLEND-003: Transition properties
Status: Implemented

Right-clicking a transition shall offer its properties and Remove. A transition between
two items may be switched between gradient seam and crossfade. Properties are:

- Gradient seam: orientation (horizontal or vertical), band width in pixels, curve
  (linear or smooth)
- Crossfade: ratio (BLEND-008)
- Feathered edge: edges, width in pixels, curve (BLEND-007)

### BLEND-007: Feathered edges
Status: Implemented

An item's context menu shall offer "Feather edges…", where the user chooses any
combination of left, right, top and bottom edges, a width in pixels (default 10% of the
item's shorter side) and a curve (linear or smooth). While the item is selected, each
feathered edge shall show a handle that can be dragged to change the width.
Feathered edges are horizontal or vertical bands on the canvas. When the item moves,
its feathered edges shall move with it; when it is resized, they shall resize with it
(width scaled in proportion). When the item is rotated (PLACE-007, PLACE-008), its
feathered edges shall stay where they are on the canvas; they do not rotate with the
item.

### BLEND-008: Crossfade ratio
Status: Implemented

A crossfade shall have a ratio from 0% to 100% (default 50%): the upper item's opacity
within the overlap.

### BLEND-004: Three or more overlapping items
Status: Implemented

Rendering composites items bottom to top in z-order. Every transition changes only one
item's transparency: the upper item of a seam or crossfade, or the feathered item
itself. When an item has several transitions, their transparencies multiply. Where three
or more items overlap, each item is composited over the result so far.

### BLEND-005: Transitions follow their items
Status: Implemented

When either item moves or resizes, its transitions shall stay attached: a seam keeps its
position relative to the overlap. When either item is rotated, a seam shall stay where it
is on the canvas. If the items stop overlapping, the transition is kept
but has no effect until they overlap again.

## Implementation notes

Decisions made where the requirements were silent; review and promote to requirements or
change as needed.

- Added images go to the right of the existing images when the canvas follows the images
  (a single image), and are centered on a fixed canvas. Dropped images are centered on the
  drop point.
- Dragging a canvas handle, or using "Canvas size…", on a canvas that follows its images
  makes it a fixed canvas, keeping the content where it is.
- Snap targets are the canvas edges and center (fixed canvases only) and every other
  image's edges and center. Snapping applies to moving images, resizing unrotated images,
  seams, crop handles on unrotated images, and canvas handles (which snap to image edges).
- A gradient seam fades the whole upper image along the blend direction, not only inside
  the overlap; the upper image stays opaque on the side of the seam its center is on. A
  crossfade applies only inside the overlap.
- After a resize that changes the aspect ratio, a feather width scales by the average of
  the width and height changes.
- Right-clicking a seam line opens the transition menu (Properties…, change kind, Remove).
  An image's menu offers creating each kind of transition with every image it overlaps,
  and opens or removes existing ones.
- The rotation handle sits 28 screen pixels above the top edge. Angles are stored to 0.1°
  and normalized to 0–360.
- A press must move 3 screen pixels before it counts as a drag; a plain click only
  selects.
- An odd quarter turn of an image whose width and height differ by an odd number of pixels
  is shifted by half a pixel so it stays on whole pixels and is not resampled.
