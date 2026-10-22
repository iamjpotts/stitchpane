# Retouching

Changing an image's own pixels. Retouching never changes the source file; it is recorded
as steps like every other edit.

## Smear tool

### SMEAR-001: Start and leave the smear tool
Status: Implemented

An item's context menu shall offer "Smear", and pressing S while an item is selected
shall do the same. While the smear tool is active, the system shall show a tool bar with
the smear kind, brush size, strength and a Done button, and a circle showing the brush
under the pointer. Done, Enter or Escape leave the tool.

### SMEAR-002: Smear kinds
Status: Implemented

The tool bar shall offer three kinds:

- **Smudge**: picks up color under the brush and drags it along the stroke, fading as
  it goes
- **Blur**: softens the pixels under the brush
- **Push**: moves pixels along the stroke, warping the image

### SMEAR-003: Brush size and strength
Status: Implemented

Brush size is a diameter in canvas pixels, by default 5% of the item's shorter side.
Strength is 1% to 100%, by default 50%. The brush is strongest at its center and fades to
nothing at its edge. Kind, size and strength the user chose are kept for the rest of
the session.

### SMEAR-004: Strokes
Status: Implemented

While the smear tool is active, dragging over the canvas shall apply a stroke to the
item the tool was started on, and only to that item; other items are not changed. The
stroke shall be shown as it is drawn. Each stroke records one step.

### SMEAR-005: Strokes stay with the image
Status: Implemented

Strokes are attached to the image, not the canvas: when the item is moved, resized,
rotated or cropped, each stroke keeps its place on the image and scales with it.
Cropping away a smeared area hides it; uncropping (CROP-005) shows it again.

## Implementation notes

Decisions made where the requirements were silent; review and promote to requirements or
change as needed.

- Strokes are stored in pixels of the upright source image, with the brush size converted
  to source pixels, and applied to the whole source before cropping and resampling.
- While a stroke is drawn, it is shown applied to the on-screen preview of the image; the
  full-resolution result replaces it shortly after the stroke ends. Export always uses
  the full-resolution result.
- The smear tool and the crop tool are not active at the same time; starting one leaves
  the other.
- A click without dragging is a stroke of one point: it blurs under the brush, and does
  nothing for smudge and push.
- Strokes on parts of the brush outside the image read and change nothing there.
