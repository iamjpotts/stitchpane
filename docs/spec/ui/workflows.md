# UI workflows

Both UI builds (GTK and iced) implement the same workflows. The GTK build should follow
GNOME conventions (header bar, primary menu).

## Main window

```
+------------------------------------------------------------------+
| [Open] [Add images] [Undo] [Redo]   Imaginer — trip.imaginer  [≡] |
+------------------------------------------------------------------+
|                                                                  |
|            canvas (zoomable, scrollable, with handles)           |
|                                                                  |
+------------------------------------------------------------------+
| 3000 × 1200 px · 300 DPI · zoom 50%   ⓘ Contains EXIF metadata   |
+------------------------------------------------------------------+
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

Open shall accept both images and `.imaginer` project files. Opening an image starts a
single-image project.

### UI-002: Unsaved changes
Status: Implemented

If the user closes a window or opens another project with unsaved changes, then the
system shall offer Save, Discard, or Cancel.

### UI-003: Zoom
Status: Implemented

The canvas shall zoom with Ctrl+scroll, Ctrl+plus, Ctrl+minus, Ctrl+0 (fit) and Ctrl+1
(100%).

### UI-004: Context menus
Status: Implemented

Right-clicking an item or transition shall open a menu of the actions that apply to it
(PLACE-005, BLEND-003, crop, resize).

## Keyboard shortcuts

| Action | Shortcut |
|---|---|
| Open | Ctrl+O |
| Save / Save as | Ctrl+S / Ctrl+Shift+S |
| Export | Ctrl+E |
| Undo / Redo | Ctrl+Z / Ctrl+Shift+Z, Ctrl+Y |
| Crop tool | C |
| Move / resize selection | Arrows / Ctrl+Arrows (Shift: 10 px) |
| Rotate selection | `[` / `]` (0.1°; Shift: 1°) |
| Disable snapping while dragging | Hold Alt |

## Implementation notes

- The GTK build uses a header bar with a main menu (File items, Edit, View, Image
  submenus), GTK file choosers and dialogs.
- The iced build has File, Edit, View and Image menus in its header, draws its dialogs
  inside the window, and uses the desktop file chooser through the XDG portal.
- iced does not report where files are dropped, so in the iced build dropped files are
  placed as "Add images…" places them.
