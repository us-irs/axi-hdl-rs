# Run every check CI runs.
all: check clippy fmt docs cross-check

check:
    cargo check --workspace
    cd axi-dma && cargo check --features portable-atomic

clippy:
    cargo clippy --workspace -- -D warnings

fmt:
    cargo fmt --all -- --check

docs:
    RUSTDOCFLAGS="--cfg docsrs -Z unstable-options --generate-link-to-definition" cargo +nightly doc --workspace --no-deps

cross-check:
    cargo build --workspace --target armv7-unknown-linux-gnueabihf
    cargo build --workspace --target armv7a-none-eabihf
