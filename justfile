# Run every check CI runs.
all: check build clippy check-fmt docs cross-check

check:
    cargo check --workspace
    cargo check -p axi-dma --features portable-atomic
    cargo check -p axi-uart16550 --features portable-atomic
    cargo check -p axi-uartlite --features portable-atomic

build:
    cargo build --workspace
    cargo build -p axi-dma --features portable-atomic
    cargo build -p axi-uart16550 --features portable-atomic
    cargo build -p axi-uartlite --features portable-atomic

clippy:
    cargo clippy --workspace -- -D warnings

fmt:
    cargo fmt --all

check-fmt:
    cargo fmt --all -- --check

docs:
    RUSTDOCFLAGS="--cfg docsrs -Z unstable-options --generate-link-to-definition" cargo +nightly doc -p axi-dma --workspace --features portable-atomic --no-deps
    RUSTDOCFLAGS="--cfg docsrs -Z unstable-options --generate-link-to-definition" cargo +nightly doc -p axi-uartlite --workspace --features portable-atomic --no-deps
    RUSTDOCFLAGS="--cfg docsrs -Z unstable-options --generate-link-to-definition" cargo +nightly doc -p axi-uart16550 --workspace --features portable-atomic --no-deps

docs-html crate:
    RUSTDOCFLAGS="--cfg docsrs -Z unstable-options --generate-link-to-definition" cargo +nightly doc -p {{crate}} --no-deps --open

cross-check:
    cargo build --workspace --target armv7-unknown-linux-gnueabihf
    cargo build --workspace --target armv7a-none-eabihf

embedded crate:
    cargo build -p {{crate}} --target armv7a-none-eabihf

build-crate crate feature="":
    cargo build -p {{crate}} --features {{feature}}

check-crate crate feature="":
    cargo check -p {{crate}} --features {{feature}}

test crate:
    cargo nextest r -p {{crate}}
    cargo test --doc -p {{crate}}

coverage crate:
    cargo llvm-cov nextest -p {{crate}}

coverage-html crate:
    cargo llvm-cov nextest -p {{crate}} --html --open
