# Single image

Editing one image. There is no separate single-image mode (UI-005): a single image is a
composition with one item whose canvas tracks the item's size, so every operation here
is also available to items in a composition.

## Crop

### CROP-001: Crop with handles
Status: Implemented

When the user activates the crop tool on an image, the system shall show a crop
rectangle with corner and edge handles over the image, and dragging a handle shall
resize the rectangle. Dragging inside the rectangle shall move it.

### CROP-002: Crop by numbers
Status: Implemented

While the crop tool is active, the system shall show the rectangle's left, top, width
and height in pixels as editable fields, kept in sync with the handles.

### CROP-003: Aspect ratio lock
Status: Implemented

The crop tool shall offer an aspect ratio choice: free (default), original, 1:1, 4:3,
3:2, 16:9, or custom. While a ratio is chosen, handles shall preserve it.

### CROP-004: Apply and cancel
Status: Implemented

When the user applies the crop (Enter or an Apply button), the system shall record a
crop step. When the user cancels (Escape), no step shall be recorded.

### CROP-005: Enlarge an earlier crop
Status: Implemented

The crop rectangle may be enlarged up to the whole source image, so an image cropped
earlier can be uncropped, however many steps ago that crop was. While the crop tool is
active, the system shall show the whole source image above the other items, with the
part outside the rectangle dimmed. Applying keeps the pixels inside the rectangle where
they were on the canvas.

## Resolution

Both meanings of "change resolution" are supported.

### RES-001: Resample pixel dimensions
Status: Implemented

The system shall let the user set new pixel width and height. Aspect ratio shall be
locked by default and may be unlocked. Applying records a resample step.

### RES-002: Resampling filter
Status: Implemented

Resampling shall use Lanczos3 by default; the user may choose nearest, bilinear, or
Lanczos3.

### RES-003: Set DPI without resampling
Status: Implemented

The system shall let the user set the DPI value only, leaving pixels unchanged.
Applying records a set-DPI step.

### RES-004: Set print size
Status: Implemented

The system shall let the user enter a print size (inches or centimeters) together with
a DPI, and shall resample to the pixel dimensions that size and DPI imply.

## Format conversion

Conversion is choosing an output format at export (see `export.md`).

### CONV-001: Readable formats
Status: Implemented

The system shall open PNG, JPEG, WebP, TIFF, BMP, GIF (first frame), HEIF/HEIC and AVIF
images.

### CONV-002: Writable formats
Status: Implemented

The system shall export PNG, JPEG, WebP, TIFF, BMP, HEIF and AVIF.

### CONV-003: Lossy quality
Status: Implemented

When exporting to a lossy format, the system shall offer a quality setting (default 90
on a 1–100 scale).

## Implementation notes

Decisions made where the requirements were silent; review and promote to requirements or
change as needed.

- The crop rectangle and its fields are in pixels of the upright source image.
- While cropping, the preview draws the image on top of the other items. Its z-order is
  unchanged once the crop is applied or cancelled.
- A custom aspect ratio is entered as width : height.
- Resampling keeps the image's top-left corner in place.
- Opening an image records an "Add image" step, so it can be undone like any other step.
- WebP export is lossy at the chosen quality. GIF is read-only (first frame).
- HEIF export needs the `libheif-plugin-x265` package and AVIF export
  `libheif-plugin-aomenc`; reading needs `libheif-plugin-libde265` and
  `libheif-plugin-aomdec`.
