# Glossary

**Canvas** — The output area of a composition, with a size in pixels and an optional DPI.
Areas not covered by any item are the canvas background.

**Composition** — A project that combines one or more items on a canvas.

**DPI** — Dots per inch stored as image metadata. Changing DPI alone does not change
pixels.

**Export** — Rendering a project to a new image file. The only way output is produced.

**Horizontal transition** — A transition between items that sit side by side (left and
right). The seam is a vertical line and the blend runs left to right. A **vertical
transition** is the same rotated: items above and below, horizontal seam.

**Item** — One placement of a source on the canvas, with its own position, size and crop.
The same source may be placed more than once.

**Project** — The sources and the ordered steps applied to them. Saved as a project file.

**Project file** — A `.imaginer` file. See `formats/project-file.md`.

**Resample** — Change an image's pixel dimensions, computing new pixels.

**Seam** — The line along which a transition is centered.

**Source** — An image file referenced by a project. Never modified.

**Step** — One recorded edit (for example "crop item", "move item"). A project is
replayed by applying its steps in order. Each completed user action produces one step.

**Transition** — A blend between two overlapping items. See `product/composition.md`.

**Transition band** — The region around the seam over which a transition blends; its
width is adjustable.

**Z-order** — The stacking order of items; higher items are drawn over lower items.
