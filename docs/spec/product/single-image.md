# Single image

Editing one image. There is no separate single-image mode (UI-005): a single image is a
composition with one item whose canvas tracks the item's size, so every operation here
is also available to items in a composition.

## Crop

### CROP-001: Crop with handles
Status: Accepted

When the user activates the crop tool on an image, the system shall show a crop
rectangle with corner and edge handles over the image, and dragging a handle shall
resize the rectangle. Dragging inside the rectangle shall move it.

### CROP-002: Crop by numbers
Status: Accepted

While the crop tool is active, the system shall show the rectangle's left, top, width
and height in pixels as editable fields, kept in sync with the handles.

### CROP-003: Aspect ratio lock
Status: Accepted

The crop tool shall offer an aspect ratio choice: free (default), original, 1:1, 4:3,
3:2, 16:9, or custom. While a ratio is chosen, handles shall preserve it.

### CROP-004: Apply and cancel
Status: Accepted

When the user applies the crop (Enter or an Apply button), the system shall record a
crop step. When the user cancels (Escape), no step shall be recorded.

## Resolution

Both meanings of "change resolution" are supported.

### RES-001: Resample pixel dimensions
Status: Accepted

The system shall let the user set new pixel width and height. Aspect ratio shall be
locked by default and may be unlocked. Applying records a resample step.

### RES-002: Resampling filter
Status: Accepted

Resampling shall use Lanczos3 by default; the user may choose nearest, bilinear, or
Lanczos3.

### RES-003: Set DPI without resampling
Status: Accepted

The system shall let the user set the DPI value only, leaving pixels unchanged.
Applying records a set-DPI step.

### RES-004: Set print size
Status: Accepted

The system shall let the user enter a print size (inches or centimeters) together with
a DPI, and shall resample to the pixel dimensions that size and DPI imply.

## Format conversion

Conversion is choosing an output format at export (see `export.md`).

### CONV-001: Readable formats
Status: Accepted

The system shall open PNG, JPEG, WebP, TIFF, BMP, GIF (first frame), HEIF/HEIC and AVIF
images.

### CONV-002: Writable formats
Status: Accepted

The system shall export PNG, JPEG, WebP, TIFF, BMP, HEIF and AVIF.

### CONV-003: Lossy quality
Status: Accepted

When exporting to a lossy format, the system shall offer a quality setting (default 90
on a 1–100 scale).
