# Export

### EXPORT-001: Export to a file
Status: Draft

The system shall render the project to a new image file whose format is chosen in the
save dialog or from the file extension (formats: CONV-002).

### EXPORT-002: DPI metadata
Status: Draft

When the project has a DPI set, export shall write it to formats that support it.

### EXPORT-003: Never overwrite sources silently
Status: Draft

If the chosen export path is one of the project's sources, then the system shall ask for
confirmation before overwriting it.

### EXPORT-004: Transparency
Status: Draft

When exporting to a format without transparency, transparent areas shall be composited
onto white.
