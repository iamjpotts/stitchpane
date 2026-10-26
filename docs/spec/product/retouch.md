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

## Light adjustments

Lightening or darkening a whole image. Three methods are offered together; each is
neutral by default, so the user uses whichever they need.

### LIGHT-001: Start, apply and cancel
Status: Implemented

An item's context menu shall offer "Adjust light", and pressing L while an item is
selected shall do the same. While the tool is active, the system shall show a tool bar
with the item's light settings (LIGHT-002 to LIGHT-004) and Reset, Cancel and Apply
buttons, and the canvas shall show the item with the settings as they are changed.
Apply (or Enter) records one step; Cancel (or Escape) leaves the item as it was. Reset
sets every setting back to its neutral value without leaving the tool.

### LIGHT-002: Exposure
Status: Implemented

Exposure, from −5 to +5 stops (EV) in steps of 0.1, by default 0, shall brighten or
darken the whole image as a camera exposure change would: each stop doubles or halves
the light. Colors that become brighter than white are clipped.

### LIGHT-003: Shadows and highlights
Status: Implemented

Shadows, from 0% to 100%, shall lighten the dark areas of the image, and Highlights,
from 0% to 100%, shall darken its bright areas, both by default 0%. Each pixel is
judged by the brightness of the area around it, Radius pixels across, by default 5% of
the image's longer side, so detail and contrast within an area are kept. Where a dark
area meets a bright one, the change shall stop at the edge rather than spill across it
as a halo.

### LIGHT-004: Levels
Status: Implemented

Levels shall map the image's values with a black point (0 to 254, by default 0), a white
point (1 to 255, by default 255) and a midtones value (0.10 to 9.99, by default 1.00):
values at or below the black point become black, values at or above the white point
become white, and those between are spread over the whole range, lighter for a midtones
value above 1 and darker below 1. The white point is always above the black point.

### LIGHT-005: Order and color
Status: Implemented

The methods shall be applied in the order exposure, shadows and highlights, levels,
after the image's smear and dodge and burn strokes. Exposure and shadows and highlights
shall change red, green and blue in the same proportion, so colors keep their hue.

### LIGHT-006: Settings stay with the image
Status: Implemented

Light settings belong to the item. Opening the tool again shall show the item's current
settings, and applying shall replace them, so adjusting again never compounds earlier
adjustments. The settings stay with the image when it is moved, resized, rotated or
cropped; the radius is in pixels of the image and scales with it.

## Dodge and burn

### DODGE-001: Start and leave the dodge and burn tool
Status: Implemented

An item's context menu shall offer "Dodge and burn", and pressing D while an item is
selected shall do the same. While the tool is active, the system shall show a tool bar
with the kind, range, brush size, strength and a Done button, and a circle showing the
brush under the pointer. Done, Enter or Escape leave the tool.

### DODGE-002: Kinds and range
Status: Implemented

**Dodge** shall lighten and **Burn** shall darken the pixels under the brush. The range
(Shadows, Midtones or Highlights) shall limit the change to dark, middle or bright
pixels, fading smoothly to none for the others. At 100% strength the change at the
brush center is one stop (EV); it fades to nothing at the brush edge. Where a stroke
crosses itself, its change does not build up.

### DODGE-003: Brush settings
Status: Implemented

The brush is by default Dodge, Midtones, 50% strength, and 5% of the item's shorter
side across. Strength is 1% to 100%. The settings the user chose are kept for the rest
of the session, separately from the smear tool's.

### DODGE-004: Strokes
Status: Implemented

Dodge and burn strokes shall behave as smear strokes do (SMEAR-004, SMEAR-005): a stroke
changes only the item the tool was started on, shows as it is drawn, records one step,
and stays with the image. Smear and dodge and burn strokes apply in the order they
were made.

## Implementation notes

Decisions made where the requirements were silent; review and promote to requirements or
change as needed.

- Light adjustments are applied to the item's pixels after cropping and resampling, at
  the size they are drawn (the radius scaled to match), so previews and export see the
  same result. Areas near a crop edge are judged by what is inside the crop.
- Shadows and highlights judges each pixel by an edge-preserving blur (a guided filter)
  of the image's brightness, and lightens or darkens by up to 2.5 stops (shadows) and 1.5
  stops (highlights) at 100%. Areas darker than middle gray get lighter; areas brighter
  than middle gray get darker.
- Levels applies to red, green and blue alike, on the image's encoded (sRGB) values.
- The light, crop, smear and dodge and burn tools are not active at the same time;
  starting one leaves the others, and a light tool left this way is cancelled.
- Dodge and burn judges the range by each pixel's own brightness.
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
