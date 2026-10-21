# Metadata

Viewing and stripping EXIF metadata. Sources are never modified (constitution 2):
stripping affects only what the project exports.

### META-001: Notice
Status: Accepted

When the selected item's source contains EXIF metadata, the status bar shall show a
notice ("Contains EXIF metadata"). With no selection, the notice shall count the items
whose sources contain EXIF ("2 images contain EXIF metadata"). The notice shall not show
for items whose metadata has been stripped.

### META-002: Open the viewer
Status: Accepted

Clicking the notice shall open the EXIF viewer for the selected item, or, with no
selection, for the first item with EXIF, with a way to switch between those items.

### META-003: Viewer contents
Status: Accepted

The viewer shall list the metadata as tag name and value pairs, read-only, grouped into
camera, capture settings, date and time, location (GPS), and other.

### META-004: Strip metadata
Status: Accepted

The viewer shall have a "Strip metadata" button. Pressing it shall record a
strip-metadata step for that item (undoable), close the viewer, and remove the item from
the notice.

### META-005: Choose metadata on export
Status: Accepted

The export dialog shall have a Metadata section with two choices:

- **Source:** None, or the EXIF of one item, listed by source file name (items whose
  metadata was stripped are not listed). The default shall be the only item's EXIF when
  the project has one item with EXIF, and None otherwise.
- **Groups:** a checkbox for each group in META-003 (camera, capture settings, date and
  time, location, other), shown only when a source is chosen. All groups shall be checked
  by default except location.

The chosen groups of the chosen EXIF shall be written to formats that support it, with
orientation and pixel dimensions updated to match the exported image.

### META-006: Apply EXIF orientation
Status: Accepted

When a source is loaded, the system shall apply its EXIF orientation so the image appears
upright. This is not a step and cannot be undone; rotation after that is done with
PLACE-007 and PLACE-008.
