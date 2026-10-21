# 0003: Image codecs

Status: Accepted

## Context

CONV-001 and CONV-002 need PNG, JPEG, WebP, TIFF, BMP, GIF, HEIF/HEIC and AVIF. HEIF is
common from phones. Library and binary size do not matter (constitution 6).

## Options

- `image` crate for PNG, JPEG, WebP, TIFF, BMP, GIF, plus `libheif-rs` (bindings to the
  system `libheif`, LGPL-3.0) for HEIF and AVIF.
- GTK's `gdk-pixbuf` loaders for reading. Not available to the iced build, so not
  suitable as the core's codec layer.

## Decision

The `image` crate for PNG, JPEG, WebP, TIFF, BMP and GIF; `libheif-rs` for HEIF and
AVIF.

## Consequences

- Building needs `libheif-dev`; running needs `libheif1` and its codec plugins (on
  Ubuntu 24.04, separate `libheif-plugin-*` packages). Confirm the libheif version
  `libheif-rs` requires against Ubuntu 24.04's when adding it.
- `libheif` is LGPL-3.0, and its HEVC encoder plugin uses x265 (GPL-2.0+). These are
  system libraries, not crates, so `cargo deny` does not see them; include them in the
  license review when a license is chosen.
