[![Crates.io](https://img.shields.io/crates/v/axi-uart16550)](https://crates.io/crates/axi-uart16550)
[![docs.rs](https://img.shields.io/docsrs/axi-uart16550)](https://docs.rs/axi-uart16550)
[![ci](https://github.com/us-irs/axi-hdl-rs/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/us-irs/axi-hdl-rs/actions/workflows/ci.yml)

AXI UART16550 driver
========

This is a native Rust driver for the
[AMD AXI UART16550 IP core](https://www.amd.com/de/products/adaptive-socs-and-fpgas/intellectual-property/axi_uart16550.html).

# Core features

- Basic driver which can be created with a given IP core base address and supports a basic
  byte-level read and write API.
- Support for [`embedded-io`](https://docs.rs/embedded-io/latest/embedded_io/) and
  [`embedded-io-async`](https://docs.rs/embedded-io-async/latest/embedded_io_async/)

# Features

If the asynchronous support for the TX side is used, the number of statically provided wakers
can be configured using the following features:

- `1-waker` which is the default
- `2-wakers`
- `4-wakers`
- `8-wakers`
- `16-wakers`
- `32-wakers`

Additionally:

- `defmt` implements `defmt::Format` for this crate's register and error types.
