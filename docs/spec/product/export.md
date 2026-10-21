# Export

### EXPORT-001: Export to a file
Status: Implemented

The system shall render the project to a new image file whose format is chosen in the
save dialog or from the file extension (formats: CONV-002).

### EXPORT-002: DPI metadata
Status: Implemented

When the project has a DPI set, export shall write it to formats that support it.

### EXPORT-003: Never overwrite sources silently
Status: Implemented

If the chosen export path is one of the project's sources, then the system shall ask for
confirmation before overwriting it.

### EXPORT-004: Transparency
Status: Implemented

When exporting to a format without transparency, transparent areas shall be composited
onto white.

## Implementation notes

- Export is refused while a source image is missing.
- DPI is written to PNG, JPEG, TIFF and BMP. WebP, HEIF and AVIF have no standard DPI
  field, so none is written.
