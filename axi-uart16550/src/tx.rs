//! # Transmitter (TX) support module
use core::convert::Infallible;

use crate::{
    DEFAULT_RX_TRIGGER_LEVEL, InvalidWakerIndex, TxAsync,
    registers::{self, FifoControl, InterruptEnable},
};
use embedded_hal_async::delay::DelayNs;

/// AXI UART16550 TX driver.
///
/// Can be created by [super::AxiUart16550::split]ting a regular AXI UARTLITE structure or
/// by [Self::steal]ing it unsafely.
pub struct Tx {
    /// Internal MMIO register structure.
    pub(crate) regs: registers::MmioRegisters<'static>,
}

impl Tx {
    /// Steal the TX part of the UART 16550.
    ///
    /// You should only use this if you can not use the regular [super::AxiUart16550] constructor
    /// and the [super::AxiUart16550::split] method.
    ///
    /// This function assumes that the setup of the UART was already done.
    /// It can be used to create a TX handle inside an interrupt handler without having to use
    /// a [critical_section::Mutex] if the user can guarantee that the TX handle will only be
    /// used by the interrupt handler, or only interrupt specific API will be used.
    ///
    /// # Safety
    ///
    /// The same safey rules specified in [super::AxiUart16550::new] apply.
    pub const unsafe fn steal(base_addr: usize) -> Self {
        Self {
            regs: unsafe { registers::Registers::new_mmio_at(base_addr) },
        }
    }

    pub(crate) fn new(regs: registers::MmioRegisters<'static>) -> Self {
        Self { regs }
    }

    /// Write a byte into the FIFO if there is space available.
    #[inline]
    pub fn write_fifo(&mut self, data: u8) -> nb::Result<(), Infallible> {
        if !self.thr_empty() {
            return Err(nb::Error::WouldBlock);
        }
        self.write_fifo_unchecked(data);
        Ok(())
    }

    /// Enable TX interrupts.
    #[inline]
    pub fn enable_interrupt(&mut self) {
        self.regs.modify_ier_or_dlm(|val| {
            let mut ier = InterruptEnable::new_with_raw_value(val);
            ier.set_thr_empty(true);
            ier.raw_value()
        });
    }

    /// Disable TX interrupts.
    #[inline]
    pub fn disable_interrupt(&mut self) {
        self.regs.modify_ier_or_dlm(|val| {
            let mut ier = InterruptEnable::new_with_raw_value(val);
            ier.set_thr_empty(false);
            ier.raw_value()
        });
    }

    /// Write into the FIFO without checking the FIFO fill status.
    ///
    /// This can be useful to completely fill the FIFO if it is known to be empty.
    #[inline(always)]
    pub fn write_fifo_unchecked(&mut self, data: u8) {
        self.regs.write_fifo_or_dll(data as u32);
    }

    /// Transmitter Holding Register empty status.
    #[inline(always)]
    pub fn thr_empty(&self) -> bool {
        self.regs.read_lsr().thr_empty()
    }

    /// Transmitter empty status.
    #[inline(always)]
    pub fn tx_empty(&self) -> bool {
        self.regs.read_lsr().tx_empty()
    }

    /// Reset the FIFOs.
    #[inline]
    pub fn reset_fifo(&mut self) {
        self.regs.write_iir_or_fcr(
            FifoControl::builder()
                .with_rx_fifo_trigger(DEFAULT_RX_TRIGGER_LEVEL)
                .with_dma_mode_sel(false)
                .with_reset_tx_fifo(true)
                .with_reset_rx_fifo(false)
                .with_fifo_enable(true)
                .build()
                .raw_value(),
        );
    }

    /// Convert this TX driver into an asynchronous TX driver.
    ///
    /// See [TxAsync::new] for more information about the `waker_idx` and `delay` arguments.
    pub fn into_async<D: DelayNs>(
        self,
        waker_idx: usize,
        delay: D,
    ) -> Result<TxAsync<D>, InvalidWakerIndex> {
        TxAsync::new(self, waker_idx, delay)
    }

    /// Should be called from the interrupt handler when a THR empty interrupt occurs.
    #[inline]
    pub fn on_interrupt_thr_empty(&mut self, next_write_chunk: &[u8]) -> usize {
        if next_write_chunk.is_empty() {
            return 0;
        }
        let mut written = 0;
        while self.thr_empty() && written < next_write_chunk.len() {
            self.write_fifo_unchecked(next_write_chunk[written]);
            written += 1;
        }
        written
    }
}

impl embedded_hal_nb::serial::ErrorType for Tx {
    type Error = Infallible;
}

impl embedded_hal_nb::serial::Write for Tx {
    #[inline]
    fn write(&mut self, word: u8) -> nb::Result<(), Self::Error> {
        self.write_fifo(word)
    }

    #[inline]
    fn flush(&mut self) -> nb::Result<(), Self::Error> {
        while !self.tx_empty() {}
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
        while !self.thr_empty() {}
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
        while !self.tx_empty() {}
        Ok(())
    }
}
