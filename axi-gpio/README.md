[![Crates.io](https://img.shields.io/crates/v/axi-gpio)](https://crates.io/crates/axi-gpio)
[![docs.rs](https://img.shields.io/docsrs/axi-gpio)](https://docs.rs/axi-gpio)
[![ci](https://github.com/us-irs/axi-hdl-rs/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/us-irs/axi-hdl-rs/actions/workflows/ci.yml)

AXI GPIO driver
========

This is a native Rust driver for the
[AMD AXI GPIO IP core](https://www.amd.com/en/products/adaptive-socs-and-fpgas/intellectual-property/axi_gpio.html).

# Core features

- Basic API to read and write GPIO pins.
- Asynchronous API to listen to events on GPIO pins.

# Features

If the asynchronous support for is used, the number of statically provided wakers
can be configured using the following features:

- `1-waker`, which is the default
- `2-wakers`
- `4-wakers`
- `8-wakers`
- `16-wakers`
- `32-wakers`

The number of required wakers is the total number of ports with interrupts enabled.
For example, when using one AXI GPIO block with interrupt support on both ports and a second
one with interrupt support on only one port, you would require at least 3 wakers, so you
could select the `4-wakers` feature.

Additionally:

- `defmt` implements `defmt::Format` for this crate's register and error types.
