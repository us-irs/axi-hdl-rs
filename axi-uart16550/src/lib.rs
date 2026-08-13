//! # AMD AXI UART16550 driver
//!
//! This is a native Rust driver for the [AMD AXI UART16550](https://www.amd.com/de/products/adaptive-socs-and-fpgas/intellectual-property/axi_uart16550.html)
//! IP core.
//!
//! # Features
//!
//! If asynchronous TX operations are used, the number of wakers  which defaults to 1 waker can
//! also be configured. The [tx_async] module provides more details on the meaning of this number.
//!
//! - `portable-atomic` enables the use of the [`portable-atomic`](https://docs.rs/portable-atomic/latest/portable_atomic/)
//!   crate for atomic operations. This is useful for platforms that do not support the standard library's atomic types.
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

use regs::fields::{FifoControl, LineControl};
pub use regs::fields::{
    InterruptEnable, InterruptId2, InterruptIdentification, LineStatus, RxFifoTrigger, StopBits,
    WordLen,
};
pub mod regs;

pub mod tx;
pub use tx::*;

pub mod tx_async;
pub use tx_async::*;

pub mod rx;
pub use rx::*;

/// Maximum FIFO depth of the AXI UART16550.
pub const FIFO_DEPTH: usize = 16;

/// Default RX FIFO trigger level.
pub const DEFAULT_RX_TRIGGER_LEVEL: RxFifoTrigger = RxFifoTrigger::EightBytes;

/// Clock configuration structure.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct ClockConfig {
    /// Divisor value.
    pub div: u16,
}

/// Divisor is zero error.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("divisor is zero")]
pub struct DivisorZeroError;

/// Calculate the error rate of the baudrate with the given clock frequency, baudrate and
/// divisor as a floating point value between 0.0 and 1.0.
#[inline]
pub fn calculate_error_rate_from_div(
    clk_in: fugit::HertzU32,
    baudrate: u32,
    div: u16,
) -> Result<f32, DivisorZeroError> {
    if baudrate == 0 || div == 0 {
        return Err(DivisorZeroError);
    }
    let actual = (clk_in.to_raw() as f32) / (16.0 * div as f32);
    Ok(libm::fabsf(actual - baudrate as f32) / baudrate as f32)
}

/// If this error occurs, the calculated baudrate divisor is too large, either because the
/// used clock is too large, or the baudrate is too slow for the used clock frequency.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("divisor too large")]
pub enum ClockConfigError {
    /// Divisor too large error.
    DivisorTooLargeError(u32),
    /// Divisor is zero error.
    DivisorZero(#[from] DivisorZeroError),
}

impl ClockConfig {
    /// New clock config with the given divisor.
    pub fn new(div: u16) -> Self {
        Self { div }
    }

    /// MSB part of the divisor.
    #[inline(always)]
    pub fn div_msb(&self) -> u8 {
        (self.div >> 8) as u8
    }

    /// LSB part of the divisor.
    #[inline(always)]
    pub fn div_lsb(&self) -> u8 {
        self.div as u8
    }

    /// This function calculates the required divisor values for a given input clock and baudrate
    /// as well as an baud error rate.
    #[inline]
    pub fn new_autocalc_with_error(
        clk_in: fugit::HertzU32,
        baudrate: u32,
    ) -> Result<(Self, f32), ClockConfigError> {
        let cfg = Self::new_autocalc(clk_in, baudrate)?;
        Ok((cfg, cfg.calculate_error_rate(clk_in, baudrate)?))
    }

    /// This function calculates the required divisor values for a given input clock and baudrate.
    ///
    /// The function will not calculate the error rate. You can use [Self::calculate_error_rate]
    /// to check the error rate, or use the [Self::new_autocalc_with_error] function to get both
    /// the clock config and its baud error.
    #[inline]
    pub fn new_autocalc(clk_in: fugit::HertzU32, baudrate: u32) -> Result<Self, ClockConfigError> {
        let div = Self::calc_div_with_integer_div(clk_in, baudrate)?;
        if div > u16::MAX as u32 {
            return Err(ClockConfigError::DivisorTooLargeError(div));
        }
        Ok(Self { div: div as u16 })
    }

    /// Calculate the error rate of the baudrate with the given clock frequency, baudrate and the
    /// current clock config as a floating point value between 0.0 and 1.0.
    #[inline]
    pub fn calculate_error_rate(
        &self,
        clk_in: fugit::HertzU32,
        baudrate: u32,
    ) -> Result<f32, DivisorZeroError> {
        calculate_error_rate_from_div(clk_in, baudrate, self.div)
    }

    /// Calculate the divisor from an input clock for a give target baudrate.
    #[inline(always)]
    pub const fn calc_div_with_integer_div(
        clk_in: fugit::HertzU32,
        baudrate: u32,
    ) -> Result<u32, DivisorZeroError> {
        if baudrate == 0 {
            return Err(DivisorZeroError);
        }
        // Rounding integer division, by adding half the divisor to the dividend.
        Ok((clk_in.to_raw() + (8 * baudrate)) / (16 * baudrate))
    }
}

/// Parity configuration.
#[derive(Default, Debug, PartialEq, Eq, Clone, Copy)]
pub enum Parity {
    /// No parity (default).
    #[default]
    None,
    /// Odd parity.
    Odd,
    /// Even parity.
    Even,
}

/// AXI UART16550 peripheral driver.
pub struct AxiUart16550 {
    rx: Rx,
    tx: Tx,
    config: UartConfig,
}

/// UART configuration structure.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct UartConfig {
    clk: ClockConfig,
    word_len: WordLen,
    parity: Parity,
    stop_bits: StopBits,
}

impl UartConfig {
    /// New with the given clock configuration.
    pub const fn new_with_clk_config(clk: ClockConfig) -> Self {
        Self {
            clk,
            word_len: WordLen::Eight,
            parity: Parity::None,
            stop_bits: StopBits::One,
        }
    }

    /// New with all parameters.
    pub const fn new(
        clk: ClockConfig,
        word_len: WordLen,
        parity: Parity,
        stop_bits: StopBits,
    ) -> Self {
        Self {
            clk,
            word_len,
            parity,
            stop_bits,
        }
    }
}

impl AxiUart16550 {
    /// Create a new AXI UART16550 peripheral driver.
    ///
    /// # Safety
    ///
    /// - The `base_addr` must be a valid memory-mapped register address of an AXI UART 16550
    ///   peripheral.
    /// - Dereferencing an invalid or misaligned address results in **undefined behavior**.
    /// - The caller must ensure that no other code concurrently modifies the same peripheral registers
    ///   in an unsynchronized manner to prevent data races.
    /// - This function does not enforce uniqueness of driver instances. Creating multiple instances
    ///   with the same `base_addr` can lead to unintended behavior if not externally synchronized.
    /// - The driver performs **volatile** reads and writes to the provided address.
    pub unsafe fn new(base_addr: u32, config: UartConfig) -> Self {
        let mut regs = unsafe { regs::Registers::new_mmio_at(base_addr as usize) };
        // This unlocks the divisor config registers.
        regs.write_lcr(LineControl::new_for_divisor_access());
        regs.write_fifo_or_dll(config.clk.div_lsb() as u32);
        regs.write_ier_or_dlm(config.clk.div_msb() as u32);
        // Configure all other settings and reset the div acess latch. This is important
        // for accessing IER and the FIFO control register again.
        regs.write_lcr(
            LineControl::builder()
                .with_div_access_latch(false)
                .with_set_break(false)
                .with_stick_parity(false)
                .with_even_parity(config.parity == Parity::Even)
                .with_parity_enable(config.parity != Parity::None)
                .with_stop_bits(config.stop_bits)
                .with_word_len(config.word_len)
                .build(),
        );
        // Disable all interrupts.
        regs.write_ier_or_dlm(InterruptEnable::new_with_raw_value(0x0).raw_value());
        // Enable FIFO, configure 8 bytes FIFO trigger by default.
        regs.write_iir_or_fcr(
            FifoControl::builder()
                .with_rx_fifo_trigger(DEFAULT_RX_TRIGGER_LEVEL)
                .with_dma_mode_sel(false)
                .with_reset_tx_fifo(true)
                .with_reset_rx_fifo(true)
                .with_fifo_enable(true)
                .build()
                .raw_value(),
        );
        Self {
            rx: Rx::new(unsafe { regs.clone() }),
            tx: Tx::new(regs),
            config,
        }
    }

    /// Raw register access.
    #[inline(always)]
    pub const fn regs(&mut self) -> &mut regs::MmioRegisters<'static> {
        &mut self.rx.regs
    }

    /// UART configuration.
    #[inline(always)]
    pub const fn config(&mut self) -> &UartConfig {
        &self.config
    }

    /// Write into the UART Lite.
    ///
    /// Returns [nb::Error::WouldBlock] if the TX FIFO is full.
    #[inline]
    pub fn write_fifo(&mut self, data: u8) -> nb::Result<(), Infallible> {
        self.tx.write_fifo(data)
    }

    /// Transmitter Holding Register empty status.
    #[inline(always)]
    pub fn thr_empty(&self) -> bool {
        self.tx.thr_empty()
    }

    /// Transmitter empty status.
    #[inline(always)]
    pub fn tx_empty(&self) -> bool {
        self.tx.tx_empty()
    }

    /// Receiver has data.
    #[inline(always)]
    pub fn rx_has_data(&self) -> bool {
        self.rx.has_data()
    }

    /// Write into the FIFO without checking the FIFO fill status.
    ///
    /// This can be useful to completely fill the FIFO if it is known to be empty.
    #[inline(always)]
    pub fn write_fifo_unchecked(&mut self, data: u8) {
        self.tx.write_fifo_unchecked(data);
    }

    /// Read the RX FIFO.
    ///
    /// This functions offers a [nb::Result] based API and returns [nb::Error::WouldBlock] if there
    /// is nothing to read.
    #[inline]
    pub fn read_fifo(&mut self) -> nb::Result<u8, Infallible> {
        self.rx.read_fifo()
    }

    /// Read from the FIFO without checking the FIFO fill status.
    #[inline(always)]
    pub fn read_fifo_unchecked(&mut self) -> u8 {
        self.rx.read_fifo_unchecked()
    }

    /// Enable interrupts according to the given interrupt enable configuration.
    #[inline(always)]
    pub fn enable_interrupts(&mut self, ier: InterruptEnable) {
        self.regs().write_ier_or_dlm(ier.raw_value());
    }

    /// Split into TX and RX halves.
    pub fn split(self) -> (Tx, Rx) {
        (self.tx, self.rx)
    }
}

impl embedded_hal_nb::serial::ErrorType for AxiUart16550 {
    type Error = Infallible;
}

impl embedded_hal_nb::serial::Write for AxiUart16550 {
    #[inline]
    fn write(&mut self, word: u8) -> nb::Result<(), Self::Error> {
        self.tx.write(word)
    }

    #[inline]
    fn flush(&mut self) -> nb::Result<(), Self::Error> {
        self.tx.flush()
    }
}

impl embedded_hal_nb::serial::Read for AxiUart16550 {
    #[inline]
    fn read(&mut self) -> nb::Result<u8, Self::Error> {
        self.rx.read()
    }
}

impl embedded_io::ErrorType for AxiUart16550 {
    type Error = Infallible;
}

impl embedded_io::Read for AxiUart16550 {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        self.rx.read(buf)
    }
}

impl embedded_io::Write for AxiUart16550 {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.tx.write(buf)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.tx.flush()
    }
}

#[cfg(test)]
mod tests {
    use crate::ClockConfigError;

    //extern crate std;
    use super::{DivisorZeroError, calculate_error_rate_from_div};

    use super::ClockConfig;
    use approx::abs_diff_eq;
    use fugit::RateExtU32;

    #[test]
    fn test_clk_calc_example_0() {
        let clk_cfg = ClockConfig::new_autocalc(100.MHz(), 56000).unwrap();
        // For some reason, the Xilinx example rounds up here..
        assert_eq!(clk_cfg.div, 0x0070);
        assert_eq!(clk_cfg.div_msb(), 0x00);
        assert_eq!(clk_cfg.div_lsb(), 0x70);
        let error = clk_cfg.calculate_error_rate(100.MHz(), 56000).unwrap();
        assert!(abs_diff_eq!(error, 0.0035, epsilon = 0.001));
        let (clk_cfg_checked, error_checked) =
            ClockConfig::new_autocalc_with_error(100.MHz(), 56000).unwrap();
        assert_eq!(clk_cfg, clk_cfg_checked);
        assert!(abs_diff_eq!(error, error_checked, epsilon = 0.001));
        let error_calc = calculate_error_rate_from_div(100.MHz(), 56000, clk_cfg.div).unwrap();
        assert!(abs_diff_eq!(error, error_calc, epsilon = 0.001));
    }

    #[test]
    fn test_clk_calc_example_1() {
        let clk_cfg = ClockConfig::new_autocalc(1843200.Hz(), 56000).unwrap();
        assert_eq!(clk_cfg.div, 0x0002);
        assert_eq!(clk_cfg.div_msb(), 0x00);
        assert_eq!(clk_cfg.div_lsb(), 0x02);
    }

    #[test]
    fn test_invalid_baud() {
        let clk_cfg = ClockConfig::new_autocalc_with_error(100.MHz(), 0);
        assert_eq!(
            clk_cfg,
            Err(ClockConfigError::DivisorZero(DivisorZeroError))
        );
    }

    #[test]
    fn test_invalid_div() {
        let error = calculate_error_rate_from_div(100.MHz(), 115200, 0);
        assert_eq!(error.unwrap_err(), DivisorZeroError);
        let error = calculate_error_rate_from_div(100.MHz(), 0, 0);
        assert_eq!(error.unwrap_err(), DivisorZeroError);
        let error = calculate_error_rate_from_div(100.MHz(), 0, 16);
        assert_eq!(error.unwrap_err(), DivisorZeroError);
    }
}
