#!/usr/bin/env bash
# rust-analyzer's check command (`rust-analyzer.check.overrideCommand` in the editor's
# settings; see docs/spec/engineering/rust.md). Runs the two `just clippy` passes,
# production code with clippy.toml and all targets with clippy/tests/clippy.toml, as one
# stream of cargo JSON messages. A diagnostic both passes report is passed on once, and
# each pass's build-finished message is dropped so neither ends the stream early.
# Lints are capped at warnings so a lint error in one crate does not stop the crates that
# depend on it from being checked.
set -uo pipefail
cd "$(dirname "$0")/.."

{
    cargo clippy --workspace --lib --bins --message-format=json -- --cap-lints warn
    CLIPPY_CONF_DIR=clippy/tests cargo clippy --workspace --all-targets --message-format=json -- --cap-lints warn
} | jq -cn --unbuffered '
    foreach inputs as $m ({seen: {}, out: null};
        if $m.reason == "compiler-message" then
            ($m.message | tojson) as $key
            | if .seen[$key] then .out = null else .seen[$key] = true | .out = $m end
        elif $m.reason == "build-finished" then .out = null
        else .out = $m end;
        .out | select(. != null))'
