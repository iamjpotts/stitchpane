# 0002: Project file format

Status: Accepted

## Context

Project files should be human-readable. `serde_yaml` is deprecated, and `serde_yml` has
an unsound/unmaintained advisory (RUSTSEC-2025-0068).

## Decision

YAML, through `yaml_serde` (maintained by the YAML organization) as a direct
dependency. `serde_yaml` is acceptable only as a transitive dependency; if it appears,
its advisory (RUSTSEC-2024-0449, via `unsafe-libyaml`) is ignored in `deny.toml` with a
comment.

## Consequences

- `yaml_serde` and `serde` are added when the project file is first implemented.
