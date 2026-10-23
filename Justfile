# iced UI tests render headlessly with the software renderer
export ICED_TEST_BACKEND := "tiny-skia"

default:
    @just -l

install:
    @cargo install --path crates/stitchpane

# run the app; pass `iced` to use the iced UI instead of GTK
run ui="gtk":
    @cargo run -p stitchpane --no-default-features --features {{ui}}

check:
    @cargo check --workspace --all-targets
    @cargo check -p stitchpane --all-targets --no-default-features --features iced

clippy:
    @cargo clippy --workspace --all-targets -- -D warnings
    @cargo clippy -p stitchpane --all-targets --no-default-features --features iced -- -D warnings

deny:
    @cargo deny check

fmt:
    @cargo +nightly fmt

fmt-check:
    @cargo +nightly fmt --check

# every Implemented requirement has a test; every test tag names a known requirement
spec-check:
    @scripts/spec-check.sh

t:
    cargo test --workspace

verify: fmt-check deny check clippy spec-check t
