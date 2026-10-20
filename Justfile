
default:
    @just -l

install:
    @cargo install --path .

check:
    @cargo check --all-targets

clippy:
    @cargo clippy

deny:
    @cargo deny check

fmt:
    @cargo +nightly fmt

fmt-check:
    @cargo +nightly fmt --check

t:
    cargo test

verify: fmt-check deny check clippy t
    
