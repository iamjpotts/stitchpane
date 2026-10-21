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
| 3000 × 1200 px · 300 DPI · zoom 50%                              |
+------------------------------------------------------------------+
```

### UI-001: Open an image or project
Status: Draft

Open shall accept both images and `.imaginer` project files. Opening an image starts a
single-image project.

### UI-002: Unsaved changes
Status: Draft

If the user closes a window or opens another project with unsaved changes, then the
system shall offer Save, Discard, or Cancel.

### UI-003: Zoom
Status: Draft

The canvas shall zoom with Ctrl+scroll, Ctrl+plus, Ctrl+minus, Ctrl+0 (fit) and Ctrl+1
(100%).

### UI-004: Context menus
Status: Draft

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
| Nudge | Arrows (Shift: 10 px) |

## Open questions

- Is there a separate "single image" mode, or is everything a composition?
