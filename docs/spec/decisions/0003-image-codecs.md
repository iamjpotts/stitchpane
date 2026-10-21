# 0003: Image codecs

Status: Proposed

## Context

CONV-001 and CONV-002 need PNG, JPEG, WebP, TIFF, BMP, GIF, HEIF/HEIC and AVIF. HEIF is
common from phones. Library and binary size do not matter (constitution 6).

## Options

- `image` crate for PNG, JPEG, WebP, TIFF, BMP, GIF, plus `libheif-rs` (bindings to the
  system `libheif`, LGPL-3.0) for HEIF and AVIF.
- GTK's `gdk-pixbuf` loaders for reading. Not available to the iced build, so not
  suitable as the core's codec layer.

## Decision

Pending.
