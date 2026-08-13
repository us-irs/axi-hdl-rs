//! # Transmitter (TX) support module
use core::convert::Infallible;

use crate::{
    InvalidWakerIndex, RxErrors, TxAsync, handle_status_reg_errors,
    registers::{self, Control, TxFifo},
};

/// AXI UARTLITE TX driver.
///
/// Can be created by [super::AxiUartlite::split]ting a regular AXI UARTLITE structure or
/// by [Self::steal]ing it unsafely.
pub struct Tx {
    pub(crate) regs: registers::MmioRegisters<'static>,
    pub(crate) errors: Option<RxErrors>,
}

impl Tx {
    /// Steal the TX part of the UART Lite.
    ///
    /// You should only use this if you can not use the regular [super::AxiUartlite] constructor
    /// and the [super::AxiUartlite::split] method.
    ///
    /// This function assumes that the setup of the UART was already done.
    /// It can be used to create a TX handle inside an interrupt handler without having to use
    /// a [critical_section::Mutex] if the user can guarantee that the TX handle will only be
    /// used by the interrupt handler, or only interrupt specific API will be used.
    ///
    /// # Safety
    ///
    /// The same safey rules specified in [super::AxiUartlite] apply.
    pub unsafe fn steal(base_addr: usize) -> Self {
        let regs = unsafe { registers::Registers::new_mmio_at(base_addr) };
        Self { regs, errors: None }
    }

    /// Write into the UART Lite.
    ///
    /// Returns [nb::Error::WouldBlock] if the TX FIFO is full.
    #[inline]
    pub fn write_fifo(&mut self, data: u8) -> nb::Result<(), Infallible> {
        let status_reg = self.regs.read_stat_reg();
        if status_reg.tx_fifo_full() {
            return Err(nb::Error::WouldBlock);
        }
        self.write_fifo_unchecked(data);
        if let Some(errors) = handle_status_reg_errors(&status_reg) {
            self.errors = Some(errors);
        }
        Ok(())
    }

    /// Reset the TX FIFO.
    #[inline]
    pub fn reset_fifo(&mut self) {
        let status = self.regs.read_stat_reg();
        self.regs.write_ctrl_reg(
            Control::builder()
                .with_enable_interrupt(status.intr_enabled())
                .with_reset_rx_fifo(false)
                .with_reset_tx_fifo(true)
                .build(),
        );
    }

    /// Write into the FIFO without checking the FIFO fill status.
    ///
    /// This can be useful to completely fill the FIFO if it is known to be empty.
    #[inline(always)]
    pub fn write_fifo_unchecked(&mut self, data: u8) {
        self.regs
            .write_tx_fifo(TxFifo::new_with_raw_value(data as u32));
    }

    /// Is the TX FIFO empty?
    #[inline(always)]
    pub fn fifo_empty(&self) -> bool {
        self.regs.read_stat_reg().tx_fifo_empty()
    }

    /// Is the TX FIFO full?
    #[inline(always)]
    pub fn fifo_full(&self) -> bool {
        self.regs.read_stat_reg().tx_fifo_full()
    }

    /// Fills the FIFO with user provided data until the user data
    /// is consumed or the FIFO is full.
    ///
    /// Returns the amount of written data, which might be smaller than the buffer size.
    pub fn fill_fifo(&mut self, buf: &[u8]) -> usize {
        let mut written = 0;
        while written < buf.len() {
            match self.write_fifo(buf[written]) {
                Ok(_) => written += 1,
                Err(nb::Error::WouldBlock) => break,
            }
        }
        written
    }

    /// Read and clear the last recorded RX errors.
    pub fn read_and_clear_last_error(&mut self) -> Option<RxErrors> {
        let errors = self.errors?;
        self.errors = None;
        Some(errors)
    }

    /// Convert this TX driver into an asynchronous TX driver.
    ///
    /// See [TxAsync::new] for more information about the `waker_idx` argument.
    pub fn into_async(self, waker_idx: usize) -> Result<TxAsync, InvalidWakerIndex> {
        TxAsync::new(self, waker_idx)
    }
}

impl embedded_hal_nb::serial::ErrorType for Tx {
    type Error = Infallible;
}

impl embedded_hal_nb::serial::Write for Tx {
    fn write(&mut self, word: u8) -> nb::Result<(), Self::Error> {
        self.write_fifo(word)
    }

    fn flush(&mut self) -> nb::Result<(), Self::Error> {
        while !self.fifo_empty() {}
        Ok(())
    }
}

impl embedded_io::ErrorType for Tx {
    type Error = Infallible;
}

impl embedded_io::Write for Tx {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() {
            return Ok(0);
        }
        while self.fifo_full() {}
        let mut written = 0;
        for &byte in buf.iter() {
            match self.write_fifo(byte) {
                Ok(_) => written += 1,
                Err(nb::Error::WouldBlock) => break,
            }
        }
        Ok(written)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        while !self.fifo_empty() {}
        Ok(())
    }
}
