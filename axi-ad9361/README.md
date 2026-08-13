[![Crates.io](https://img.shields.io/crates/v/axi-ad9361)](https://crates.io/crates/axi-ad9361)
[![docs.rs](https://img.shields.io/docsrs/axi-ad9361)](https://docs.rs/axi-ad9361)
[![ci](https://github.com/us-irs/axi-hdl-rs/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/us-irs/axi-hdl-rs/actions/workflows/ci.yml)

AXI AD9361 driver
========

This is a native Rust driver for the Analog Devices
[AXI AD9361 IP core](https://analogdevicesinc.github.io/hdl/library/axi_ad9361/index.html), an
FPGA-fabric IP core that implements the CMOS/LVDS digital interface expected by an AD9361 RF
transceiver, exposing separate ADC and DAC datapaths.

# Core features

- Basic driver which can be created with a given IP core base address, and hands out at most one
  ADC and one DAC driver instance, each covering the register sub-block for that datapath.
- ADC/DAC bring-up (reset, R1 mode, channel enable) and access to the shared, ADC and DAC
  register blocks.

# Features

- `defmt` implements `defmt::Format` for this crate's register types.
