#!/usr/bin/env bash
# Traceability check between docs/spec and tests (conventions: docs/spec/README.md).
#   - every requirement with "Status: Implemented" is referenced by at least one test, or
#     names the tooling that enforces it in an "Enforced by:" line (coding standards)
#   - every requirement ID referenced by a test exists in the spec
# Requirements are headings like `### CROP-001: Title`; tests reference them as cucumber
# tags (`@CROP-001`) in .feature files or as `spec: CROP-001` in Rust sources.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

id='[A-Z]+-[0-9]{3}'

all=$(grep -rhoE "^### $id:" docs/spec | grep -oE "$id" | sort -u)
implemented=$(find docs/spec -name '*.md' -print0 | xargs -0 awk -v re="^### $id:" '
    $0 ~ re { split($2, a, ":"); current = a[1]; next }
    /^#/ { current = "" }
    /^Status: Implemented/ && current != "" { print current; current = "" }
' | sort -u)
enforced=$(find docs/spec -name '*.md' -print0 | xargs -0 awk -v re="^### $id:" '
    $0 ~ re { split($2, a, ":"); current = a[1]; next }
    /^#/ { current = "" }
    /^Enforced by: ./ && current != "" { print current; current = "" }
' | sort -u)
referenced=$( {
    grep -rhoE "@$id" --include='*.feature' crates || true
    grep -rhoE "spec: $id" --include='*.rs' crates || true
} | grep -oE "$id" | sort -u)

status=0
untested=$(comm -23 <(echo "$implemented") <(sort -u <(echo "$referenced") <(echo "$enforced")) | grep . || true)
unknown=$(comm -13 <(echo "$all") <(echo "$referenced") | grep . || true)
if [[ -n "$untested" ]]; then
    echo "Implemented requirements with no test:"; echo "$untested" | sed 's/^/  /'; status=1
fi
if [[ -n "$unknown" ]]; then
    echo "Tests reference unknown requirement IDs:"; echo "$unknown" | sed 's/^/  /'; status=1
fi
echo "spec-check: $(echo "$all" | grep -c .) requirements, $(echo "$implemented" | grep -c .) implemented, $(echo "$referenced" | grep -c .) referenced by tests"
exit $status
