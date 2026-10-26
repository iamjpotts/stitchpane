
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

# production code with clippy.toml, then all targets with clippy/tests/clippy.toml (RUST-001)
clippy:
    @cargo clippy --workspace --lib --bins -- -D warnings
    @cargo clippy -p stitchpane --lib --bins --no-default-features --features iced -- -D warnings
    @test -f clippy/tests/clippy.toml
    @CLIPPY_CONF_DIR=clippy/tests cargo clippy --workspace --all-targets -- -D warnings
    @CLIPPY_CONF_DIR=clippy/tests cargo clippy -p stitchpane --all-targets --no-default-features --features iced -- -D warnings

deny:
    @cargo deny check

# refresh cargo's crates.io index cache, which `cargo deny` reads for yanks but never updates
index-refresh:
    @cargo update --dry-run --quiet

fmt:
    @cargo +nightly fmt

fmt-check:
    @cargo +nightly fmt --check

# every Implemented requirement has a test; every test tag names a known requirement
spec-check:
    @scripts/spec-check.sh

t:
    cargo test --workspace

verify: fmt-check index-refresh deny check clippy spec-check t
