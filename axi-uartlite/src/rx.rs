//! # Receiver (RX) support module
use core::convert::Infallible;

use crate::regs::{self, Registers, fields::Status};

/// RX error structure which tracks if an error has occurred.
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct RxErrors {
    parity: bool,
    frame: bool,
    overrun: bool,
}

impl RxErrors {
    /// Create a new empty RX error structure.
    pub const fn new() -> Self {
        Self {
            parity: false,
            frame: false,
            overrun: false,
        }
    }

    /// Parity error occurred.
    pub const fn parity(&self) -> bool {
        self.parity
    }

    /// Frame error occurred.
    pub const fn frame(&self) -> bool {
        self.frame
    }

    /// Overrun error occurred.
    pub const fn overrun(&self) -> bool {
        self.overrun
    }

    /// Any error has occurred.
    pub const fn has_errors(&self) -> bool {
        self.parity || self.frame || self.overrun
    }
}

/// AXI UARTLITE TX driver.
///
/// Can be created by [super::AxiUartlite::split]ting a regular AXI UARTLITE structure or
/// by [Self::steal]ing it unsafely.
pub struct Rx {
    pub(crate) regs: regs::MmioRegisters<'static>,
    pub(crate) errors: Option<RxErrors>,
}

impl Rx {
    /// Steal the RX part of the UART Lite.
    ///
    /// You should only use this if you can not use the regular [super::AxiUartlite] constructor
    /// and the [super::AxiUartlite::split] method.
    ///
    /// This function assumes that the setup of the UART was already done.
    /// It can be used to create an RX handle inside an interrupt handler without having to use
    /// a [critical_section::Mutex] if the user can guarantee that the RX handle will only be
    /// used by the interrupt handler or only interrupt specific API will be used.
    ///
    /// # Safety
    ///
    /// The same safey rules specified in [super::AxiUartlite] apply.
    #[inline]
    pub const unsafe fn steal(base_addr: usize) -> Self {
        Self {
            regs: unsafe { Registers::new_mmio_at(base_addr) },
            errors: None,
        }
    }

    /// Read the RX FIFO.
    ///
    /// This functions offers a [nb::Result] based API and returns [nb::Error::WouldBlock] if there
    /// is nothing to read.
    #[inline]
    pub fn read_fifo(&mut self) -> nb::Result<u8, Infallible> {
        let status_reg = self.regs.read_stat_reg();
        if !status_reg.rx_fifo_valid_data() {
            return Err(nb::Error::WouldBlock);
        }
        let val = self.read_fifo_unchecked();
        if let Some(errors) = handle_status_reg_errors(&status_reg) {
            self.errors = Some(errors);
        }
        Ok(val)
    }

    /// Read from the FIFO without checking the FIFO fill status.
    #[inline(always)]
    pub fn read_fifo_unchecked(&mut self) -> u8 {
        self.regs.read_rx_fifo().data()
    }

    /// Does the RX FIFO have valid data?
    #[inline(always)]
    pub fn has_data(&self) -> bool {
        self.regs.read_stat_reg().rx_fifo_valid_data()
    }

    /// This simply reads all available bytes in the RX FIFO.
    ///
    /// It returns the number of read bytes.
    #[inline]
    pub fn read_whole_fifo(&mut self, buf: &mut [u8; 16]) -> usize {
        let mut read = 0;
        while read < buf.len() {
            match self.read_fifo() {
                Ok(byte) => {
                    buf[read] = byte;
                    read += 1;
                }
                Err(nb::Error::WouldBlock) => break,
            }
        }
        read
    }

    /// Can be called in the interrupt handler for the UART Lite to handle RX reception.
    ///
    /// Simply calls [Rx::read_whole_fifo].
    #[inline]
    pub fn on_interrupt_rx(&mut self, buf: &mut [u8; 16]) -> usize {
        self.read_whole_fifo(buf)
    }

    /// Read and clear the last RX errors.
    ///
    /// Returns [None] if no errors have occured.
    pub fn read_and_clear_last_error(&mut self) -> Option<RxErrors> {
        let errors = self.errors?;
        self.errors = None;
        Some(errors)
    }
}

impl embedded_hal_nb::serial::ErrorType for Rx {
    type Error = Infallible;
}

impl embedded_hal_nb::serial::Read for Rx {
    #[inline]
    fn read(&mut self) -> nb::Result<u8, Self::Error> {
        self.read_fifo()
    }
}

impl embedded_io::ErrorType for Rx {
    type Error = Infallible;
}

impl embedded_io::Read for Rx {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() {
            return Ok(0);
        }
        while !self.has_data() {}
        let mut read = 0;
        for byte in buf.iter_mut() {
            match self.read_fifo() {
                Ok(data) => {
                    *byte = data;
                    read += 1;
                }
                Err(nb::Error::WouldBlock) => break,
            }
        }
        Ok(read)
    }
}

/// Extract RX errors from the status register.
pub const fn handle_status_reg_errors(status_reg: &Status) -> Option<RxErrors> {
    let mut errors = RxErrors::new();
    if status_reg.frame_error() {
        errors.frame = true;
    }
    if status_reg.parity_error() {
        errors.parity = true;
    }
    if status_reg.overrun_error() {
        errors.overrun = true;
    }
    if !errors.has_errors() {
        return None;
    }
    Some(errors)
}
