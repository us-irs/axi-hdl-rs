AXI DMA driver
========

This is a native Rust driver for the
[AMD AXI DMA IP core](https://www.amd.com/en/products/adaptive-socs-and-fpgas/intellectual-property/axi_dma.html).

# Core features

- Basic driver which can be created with a given IP core base address, supporting blocking and
  interrupt-driven MM2S (write/TX) and S2MM (read/RX) transfers in direct-register (simple) mode.
- Support for [`embedded-io`](https://docs.rs/embedded-io/latest/embedded_io/) and
  [`embedded-io-async`](https://docs.rs/embedded-io-async/latest/embedded_io_async/)

# Features

If the asynchronous support for the MM2S (write/TX) side is used, the number of statically
provided wakers can be configured using the following features:

- `1-waker` which is the default
- `2-wakers`
- `4-wakers`
- `8-wakers`
- `16-wakers`
- `32-wakers`

Additionally:

- `portable-atomic` uses the [`portable-atomic`](https://docs.rs/portable-atomic) crate's atomic
  types instead of `core::sync::atomic`, for targets/configurations without native atomics.
- `defmt` implements `defmt::Format` for this crate's register and error types.
