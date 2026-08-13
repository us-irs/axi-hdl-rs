//! # AXI UART Lite v2.0 driver
//!
//! This is a native Rust driver for the
//! [AMD AXI UART Lite v2.0 IP core](https://www.amd.com/en/products/adaptive-socs-and-fpgas/intellectual-property/axi_uartlite.html).
//!
//! # Special not on Zynq7000 usage
//!
//! When using this on the Zynq7000 platform, you might have to re-configure the interrupt sensitivity
//! in the GIC. An example can be found [here](https://egit.irs.uni-stuttgart.de/rust/zynq7000-rs/src/commit/1ab64050974242e43a7c5a2df5fb09256bc06274/firmware/examples/zedboard/src/bin/uart-non-blocking.rs#L189).
//!
//! # Features
//!
//! If asynchronous TX operations are used, the number of wakers  which defaults to 1 waker can
//! also be configured. The [tx_async] module provides more details on the meaning of this number.
//!
//! - `portable-atomic` enables the use of the [`portable-atomic`](https://docs.rs/portable-atomic/latest/portable_atomic/)
//!   crate for atomic operations. This is useful for platforms that do not support the standard library's atomic types.
//! - `defmt` implements `defmt::Format` for this crate's register and error types.
//! - `1-waker` which is also a `default` feature
//! - `2-wakers`
//! - `4-wakers`
//! - `8-wakers`
//! - `16-wakers`
//! - `32-wakers`
#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(missing_docs)]

use core::convert::Infallible;
use regs::fields::Control;
pub use regs::fields::Status;
pub mod regs;

pub mod tx;
pub use tx::*;

pub mod rx;
pub use rx::*;

pub mod tx_async;
pub use tx_async::*;

/// Maximum FIFO depth of the AXI UART Lite.
pub const FIFO_DEPTH: usize = 16;

/// RX error structure.
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct RxErrorsCounted {
    parity: u8,
    frame: u8,
    overrun: u8,
}

impl RxErrorsCounted {
    /// Create a new empty RX error counter.
    pub const fn new() -> Self {
        Self {
            parity: 0,
            frame: 0,
            overrun: 0,
        }
    }

    /// Parity error count.
    pub const fn parity(&self) -> u8 {
        self.parity
    }

    /// Frame error count.
    pub const fn frame(&self) -> u8 {
        self.frame
    }

    /// Overrun error count.
    pub const fn overrun(&self) -> u8 {
        self.overrun
    }

    /// Some error has occurred.
    pub fn has_errors(&self) -> bool {
        self.parity > 0 || self.frame > 0 || self.overrun > 0
    }
}

/// AXI UART Lite peripheral driver.
pub struct AxiUartlite {
    rx: Rx,
    tx: Tx,
    errors: RxErrorsCounted,
}

impl AxiUartlite {
    /// Create a new AXI UART Lite peripheral driver.
    ///
    /// # Safety
    ///
    /// - The `base_addr` must be a valid memory-mapped register address of an AXI UART Lite peripheral.
    /// - Dereferencing an invalid or misaligned address results in **undefined behavior**.
    /// - The caller must ensure that no other code concurrently modifies the same peripheral registers
    ///   in an unsynchronized manner to prevent data races.
    /// - This function does not enforce uniqueness of driver instances. Creating multiple instances
    ///   with the same `base_addr` can lead to unintended behavior if not externally synchronized.
    /// - The driver performs **volatile** reads and writes to the provided address.
    pub const unsafe fn new(base_addr: u32) -> Self {
        let regs = unsafe { regs::Registers::new_mmio_at(base_addr as usize) };
        Self {
            rx: Rx {
                regs: unsafe { regs.clone() },
                errors: None,
            },
            tx: Tx { regs, errors: None },
            errors: RxErrorsCounted::new(),
        }
    }

    /// Direct register access.
    #[inline(always)]
    pub const fn regs(&mut self) -> &mut regs::MmioRegisters<'static> {
        &mut self.tx.regs
    }

    /// Write into the UART Lite.
    ///
    /// Returns [nb::Error::WouldBlock] if the TX FIFO is full.
    #[inline]
    pub fn write_fifo(&mut self, data: u8) -> nb::Result<(), Infallible> {
        self.tx.write_fifo(data)?;
        if let Some(errors) = self.tx.errors {
            self.handle_status_reg_errors(errors);
        }
        Ok(())
    }

    /// Write into the FIFO without checking the FIFO fill status.
    ///
    /// This can be useful to completely fill the FIFO if it is known to be empty.
    #[inline(always)]
    pub fn write_fifo_unchecked(&mut self, data: u8) {
        self.tx.write_fifo_unchecked(data);
    }

    /// Read from the UART Lite.
    ///
    /// Offers a
    #[inline]
    pub fn read_fifo(&mut self) -> nb::Result<u8, Infallible> {
        let val = self.rx.read_fifo()?;
        if let Some(errors) = self.rx.errors {
            self.handle_status_reg_errors(errors);
        }
        Ok(val)
    }

    /// Read from the FIFO without checking the FIFO fill status.
    #[inline(always)]
    pub fn read_fifo_unchecked(&mut self) -> u8 {
        self.rx.read_fifo_unchecked()
    }

    /// Is the TX FIFO empty?
    #[inline(always)]
    pub fn tx_fifo_empty(&self) -> bool {
        self.tx.fifo_empty()
    }

    /// TX FIFO full status.
    #[inline(always)]
    pub fn tx_fifo_full(&self) -> bool {
        self.tx.fifo_full()
    }

    /// RX FIFO has data.
    #[inline(always)]
    pub fn rx_has_data(&self) -> bool {
        self.rx.has_data()
    }

    /// Read the error counters and also resets them.
    pub fn read_and_clear_errors(&mut self) -> RxErrorsCounted {
        let errors = self.errors;
        self.errors = RxErrorsCounted::new();
        errors
    }

    #[inline(always)]
    fn handle_status_reg_errors(&mut self, errors: RxErrors) {
        if errors.frame() {
            self.errors.frame = self.errors.frame.saturating_add(1);
        }
        if errors.parity() {
            self.errors.parity = self.errors.parity.saturating_add(1);
        }
        if errors.overrun() {
            self.errors.overrun = self.errors.overrun.saturating_add(1);
        }
    }

    /// Reset the RX FIFO.
    #[inline]
    pub fn reset_rx_fifo(&mut self) {
        self.regs().write_ctrl_reg(
            Control::builder()
                .with_enable_interrupt(false)
                .with_reset_rx_fifo(true)
                .with_reset_tx_fifo(false)
                .build(),
        );
    }

    /// Reset the TX FIFO.
    #[inline]
    pub fn reset_tx_fifo(&mut self) {
        self.regs().write_ctrl_reg(
            Control::builder()
                .with_enable_interrupt(false)
                .with_reset_rx_fifo(false)
                .with_reset_tx_fifo(true)
                .build(),
        );
    }

    /// Split the driver into [Tx] and [Rx] halves.
    #[inline]
    pub fn split(self) -> (Tx, Rx) {
        (self.tx, self.rx)
    }

    /// Enable UART Lite interrupts.
    #[inline]
    pub fn enable_interrupt(&mut self) {
        self.regs().write_ctrl_reg(
            Control::builder()
                .with_enable_interrupt(true)
                .with_reset_rx_fifo(false)
                .with_reset_tx_fifo(false)
                .build(),
        );
    }

    /// Disable UART Lite interrupts.
    #[inline]
    pub fn disable_interrupt(&mut self) {
        self.regs().write_ctrl_reg(
            Control::builder()
                .with_enable_interrupt(false)
                .with_reset_rx_fifo(false)
                .with_reset_tx_fifo(false)
                .build(),
        );
    }
}

impl embedded_hal_nb::serial::ErrorType for AxiUartlite {
    type Error = Infallible;
}

impl embedded_hal_nb::serial::Write for AxiUartlite {
    #[inline]
    fn write(&mut self, word: u8) -> nb::Result<(), Self::Error> {
        self.tx.write(word)
    }

    #[inline]
    fn flush(&mut self) -> nb::Result<(), Self::Error> {
        self.tx.flush()
    }
}

impl embedded_hal_nb::serial::Read for AxiUartlite {
    #[inline]
    fn read(&mut self) -> nb::Result<u8, Self::Error> {
        self.rx.read()
    }
}

impl embedded_io::ErrorType for AxiUartlite {
    type Error = Infallible;
}

impl embedded_io::Read for AxiUartlite {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        self.rx.read(buf)
    }
}

impl embedded_io::Write for AxiUartlite {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.tx.write(buf)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.tx.flush()
    }
}
