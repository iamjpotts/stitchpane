# UI workflows

Both UI builds (GTK and iced) implement the same workflows. The GTK build should follow
GNOME conventions (header bar, primary menu).

## Main window

```
+------------------------------------------------------------------------+
| [Open] [Add] [Save] [Undo] [Redo]   Stitch Pane — trip.stitchpane  [≡] |
+------------------------------------------------------------------------+
|                                                                        |
|              canvas (zoomable, scrollable, with handles)               |
|                                                                        |
+------------------------------------------------------------------------+
| 3000 × 1200 px · 300 DPI · zoom 50%   ⓘ Contains EXIF metadata         |
+------------------------------------------------------------------------+
```

### UI-005: One interface
Status: Implemented

Single images and compositions shall use the same window and tools; there is no
separate single-image mode. Revisit if this proves awkward in use.

### UI-006: Fine adjustment with the keyboard
Status: Implemented

The arrow keys shall adjust whatever is selected by 1 pixel, or 10 pixels with Shift:

- an item, crop rectangle, seam, or feathered edge: arrows move it
- an item or crop rectangle with Ctrl held: arrows resize it, moving the right or bottom
  edge
- an item: `[` and `]` rotate it by 0.1° counterclockwise and clockwise (1° with Shift)

Each run of key presses on the same target records one step when the user moves on to
something else (UNDO-004).

### UI-001: Open an image or project
Status: Implemented

Open shall accept both images and `.stitchpane` project files. Opening an image starts a
single-image project.

### UI-002: Unsaved changes
Status: Implemented

If the user closes a window or opens another project with unsaved changes, then the
system shall offer Save, Discard, or Cancel. Opening an image or project is not a
change; adding images, including to an empty canvas and by dropping files, is.

### UI-003: Zoom
Status: Implemented

The canvas shall zoom with Ctrl+scroll, Ctrl+plus, Ctrl+minus, Ctrl+0 (fit) and Ctrl+1
(100%).

### UI-004: Context menus
Status: Implemented

Right-clicking an item or transition shall open a menu of the actions that apply to it
(PLACE-005, BLEND-003, crop, resize).

### UI-007: File dialogs start in the last folder
Status: Implemented

When a file dialog opens, it shall start in the folder of the file most recently picked
in a file dialog, opened, saved or exported during this session. Before any, it starts
where the desktop's file chooser starts by default.

### UI-008: Save button
Status: Implemented

The header bar shall have a Save button next to Open and Add images. It shall do what
File > Save does: save to the project's file, or ask for a file name if it has none.

### UI-009: Handles keep up while resizing
Status: Implemented

While the user drags an item, crop or canvas handle, the outline, handles and snapping
guides shall follow the pointer without waiting for images to be resampled. Resampling
an image to its new size shall happen in the background; until it is ready, the image
is drawn stretched from its previous resample.

### UI-010: Zoom menu in the status bar
Status: Implemented

Clicking the zoom level in the status bar shall open a menu with the same zoom choices
as the View menu.

### UI-011: The canvas never blanks while redrawing
Status: Implemented

When the canvas image changes, the system shall keep showing the previous canvas image
until a newer one is ready to draw, so the canvas never shows an empty or partly drawn
frame. When a canvas image becomes ready, the system shall show it only if it is newer
than the one shown; an older image is skipped.

### UI-012: The canvas keeps pace while it changes
Status: Implemented

While earlier canvas images are still being prepared, the system shall start preparing
each new canvas image without waiting for them, up to a small fixed number at a time,
so the time to prepare one image limits how far the canvas lags behind the pointer, not
how often the canvas updates. If that number is reached, the system shall keep only the
newest waiting image and start it when one finishes.

### UI-013: Quit
Status: Implemented

The main menu shall offer Quit, also Ctrl+Q, as the last item of the menu that holds
Open and Save. Quitting shall do what closing the window does, offering Save, Discard
or Cancel first if there are unsaved changes (UI-002).

## Keyboard shortcuts

| Action | Shortcut |
|---|---|
| Open | Ctrl+O |
| Save / Save as | Ctrl+S / Ctrl+Shift+S |
| Export | Ctrl+E |
| Quit | Ctrl+Q |
| Undo / Redo | Ctrl+Z / Ctrl+Shift+Z, Ctrl+Y |
| Crop tool | C |
| Smear tool | S |
| Adjust light | L |
| Dodge and burn tool | D |
| Remove selected image | Delete, Backspace |
| Move / resize selection | Arrows / Ctrl+Arrows (Shift: 10 px) |
| Rotate selection | `[` / `]` (0.1°; Shift: 1°) |
| Disable snapping while dragging | Hold Alt |

## Implementation notes

- The GTK build uses a header bar with a main menu (File items, Edit, View, Image
  submenus), GTK file choosers and dialogs.
- Quit (UI-013) is the last item of the GTK main menu, after a separator (GNOME
  convention), and the last item of the iced File menu.
- The iced build has File, Edit, View and Image menus in its header, draws its dialogs
  inside the window, and uses the desktop file chooser through the XDG portal.
- iced does not report where files are dropped, so in the iced build dropped files are
  placed as "Add images…" places them.
- The last folder (UI-007) is not remembered after quitting.
- Background resampling (UI-009) applies to the on-screen preview only; export always
  resamples at full quality before writing.
- The iced renderer uploads large images to the GPU in the background and draws nothing
  for an image until its upload finishes, so the iced build swaps in a new canvas image
  only once iced reports it allocated (UI-011), and keeps several uploads in flight
  (UI-012). The GTK build draws each canvas image synchronously. See
  `decisions/0007-iced-canvas-uploads.md`.
- The light bar (LIGHT-001) has two rows in both builds, so it fits the default window:
  exposure, shadows, highlights and radius, then levels and the buttons. The GTK bar's
  black and white point ranges follow each other so they cannot cross; the iced bar
  leaves the settings unchanged while a field is not a number or is out of range.
