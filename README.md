
Image editor focused on ease of use.

# Features

## Single Image

* Crop
* Convert format
* Change resolution

## Combining Multiple Images

* Create an image by combining other images (even if varying sizes, aspect ratios, or resolutions)
* Position
* Resize
* Crop
* Blend transitions - horizontal or vertical, click-and-drag to move or change width; right-click menu to change other properties

## Project Files

References to source image(s) and actions applied can be saved to a .stitchpane file.

It is a yaml file that declares the source images, and the sequence of steps/transitions applied.

It does not contain the source image data, only references and steps.

# Supported Operating Systems

Ubuntu 24.04 or later.

# Build Dependencies

`libgtk-4-dev` (for the default GTK build), `libheif-dev`, `jq`.

# Runtime Dependencies

`libgtk-4-1` (for the default GTK build), `libheif1`, and the libheif plugins for the HEIF and AVIF codecs: `libheif-plugin-libde265`, `libheif-plugin-x265`, `libheif-plugin-aomdec`, `libheif-plugin-aomenc`.

# Building

`just install` builds the GTK version. `just run iced` runs the iced version.

# Usage

`stitchpane [image or project files…]`. Several images start a composition. Drag images to
move them, use the handles to resize and rotate, and right-click images, seams and the
canvas for more. The arrow keys, `[` and `]` make fine adjustments.
