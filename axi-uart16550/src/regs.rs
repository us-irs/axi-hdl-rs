//! # Raw register module

/// Register field types.
pub mod fields {
    use arbitrary_int::u2;

    /// Transmitter Holding Register.
    #[bitbybit::bitfield(u32, defmt_bitfields(feature = "defmt"))]
    pub struct Fifo {
        /// Bytes to transmit or receive.
        #[bits(0..=7, rw)]
        data: u8,
    }

    /// Interrupt Enable Register.
    #[bitbybit::bitfield(u32, defmt_bitfields(feature = "defmt"))]
    pub struct InterruptEnable {
        /// Enable Modem Status Interrupt
        #[bit(3, rw)]
        modem_status: bool,
        /// Enable Receiver Line Status Interrupt
        #[bit(2, rw)]
        line_status: bool,
        /// Enable Transmitter Holding Register Empty Interrupt
        #[bit(1, rw)]
        thr_empty: bool,
        /// Enable Received Data Available Interrupt
        #[bit(0, rw)]
        rx_avl: bool,
    }

    /// Interrupt identification ID
    #[bitbybit::bitenum(u3, exhaustive = false)]
    #[derive(Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum InterruptId2 {
        /// Receiver Line Status.
        ReceiverLineStatus = 0b011,
        /// RX data available.
        RxDataAvailable = 0b010,
        /// Character timeout.
        CharTimeout = 0b110,
        /// THR empty.
        ThrEmpty = 0b001,
        /// Modem status.
        ModemStatus = 0b000,
    }

    /// Interrupt Identification Register
    #[bitbybit::bitfield(u32, defmt_bitfields(feature = "defmt"))]
    pub struct InterruptIdentification {
        /// 16550 mode enabled?
        #[bits(6..=7, r)]
        fifo_enabled: u2,
        /// Interrupt ID2.
        #[bits(1..=3, r)]
        int_id: Option<InterruptId2>,
        /// Interrupt Pending, active low.
        #[bit(0, r)]
        int_pend_n: bool,
    }

    /// RX FIFO trigger level.
    #[bitbybit::bitenum(u2, exhaustive = true)]
    #[derive(Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum RxFifoTrigger {
        /// One byte.
        OneByte = 0b00,
        /// 4 bytes.
        FourBytes = 0b01,
        /// 8 bytes.
        EightBytes = 0b10,
        /// 14 bytes.
        FourteenBytes = 0b11,
    }

    impl RxFifoTrigger {
        /// Raw number instead of register value.
        pub const fn as_num(self) -> u32 {
            match self {
                RxFifoTrigger::OneByte => 1,
                RxFifoTrigger::FourBytes => 4,
                RxFifoTrigger::EightBytes => 8,
                RxFifoTrigger::FourteenBytes => 14,
            }
        }
    }

    /// FIFO Control Register
    #[bitbybit::bitfield(u32, default = 0x0, defmt_bitfields(feature = "defmt"))]
    pub struct FifoControl {
        /// RX FIFO trigger level.
        #[bits(4..=5, rw)]
        rx_fifo_trigger: RxFifoTrigger,
        /// DMA mode select.
        #[bit(3, rw)]
        dma_mode_sel: bool,
        /// Reset TX FIFO.
        #[bit(2, rw)]
        reset_tx_fifo: bool,
        /// Reset RX FIFO.
        #[bit(1, rw)]
        reset_rx_fifo: bool,
        /// FIFO enable.
        #[bit(0, rw)]
        fifo_enable: bool,
    }

    /// Word length in bits.
    #[bitbybit::bitenum(u2, exhaustive = true)]
    #[derive(Default, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum WordLen {
        /// 5 bits.
        Five = 0b00,
        /// 6 bits.
        Six = 0b01,
        /// 7 bits.
        Seven = 0b10,
        /// 8 bits (default).
        #[default]
        Eight = 0b11,
    }

    /// Stop bits.
    #[bitbybit::bitenum(u1, exhaustive = true)]
    #[derive(Default, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum StopBits {
        /// One stop bit (default).
        #[default]
        One = 0b0,
        /// 1.5 for 5 bits/char, 2 otherwise.
        OnePointFiveOrTwo = 0b1,
    }

    /// Line control register.
    #[bitbybit::bitfield(u32, default = 0x00, defmt_bitfields(feature = "defmt"))]
    pub struct LineControl {
        /// Divisor Latch Access Bit.
        #[bit(7, rw)]
        div_access_latch: bool,
        /// Set break bit.
        #[bit(6, rw)]
        set_break: bool,
        /// Parity stick bit.
        #[bit(5, rw)]
        stick_parity: bool,
        /// Even parity.
        #[bit(4, rw)]
        even_parity: bool,
        /// Parity enable.
        #[bit(3, rw)]
        parity_enable: bool,
        /// 0: 1 stop bit, 1: 2 stop bits or 1.5 if 5 bits/char selected
        #[bit(2, rw)]
        stop_bits: StopBits,
        /// Word length.
        #[bits(0..=1, rw)]
        word_len: WordLen,
    }

    impl LineControl {
        /// New line control value for accessing divisor latches.
        pub fn new_for_divisor_access() -> Self {
            Self::new_with_raw_value(0x80)
        }
    }

    /// Line Status Register
    #[bitbybit::bitfield(u32, defmt_bitfields(feature = "defmt"))]
    #[derive(Debug)]
    pub struct LineStatus {
        /// Error in RX FIFO.
        #[bit(7, rw)]
        error_in_rx_fifo: bool,
        /// In the FIFO mode, this is set to 1 when the TX FIFO and shift register are both empty.
        #[bit(6, rw)]
        tx_empty: bool,
        /// In the FIFO mode, this is set to 1 when the TX FIFO is empty. There might still be a byte
        /// in the TX shift register.
        #[bit(5, rw)]
        thr_empty: bool,
        /// Break interrupt.
        #[bit(4, rw)]
        break_interrupt: bool,
        /// Framing error.
        #[bit(3, rw)]
        framing_error: bool,
        /// Parity error.
        #[bit(2, rw)]
        parity_error: bool,
        /// Overrun error.
        #[bit(1, rw)]
        overrun_error: bool,
        /// Data ready.
        #[bit(0, rw)]
        data_ready: bool,
    }
}

/// Raw register block.
#[derive(derive_mmio::Mmio)]
#[repr(C)]
pub struct Registers {
    _reserved: [u32; 0x400],
    /// FIFO register for LCR[7] == 0 or Divisor Latch (LSB) register for LCR[7] == 1
    fifo_or_dll: u32,
    /// Interrupt Enable Register for LCR[7] == 0 or Divisor Latch (MSB) register for LCR[7] == 1
    ier_or_dlm: u32,
    /// Interrupt Identification Register or FIFO Control Register. FCR is not included in 16450
    /// mode. If LCR[7] == 1, this register will be the read-only FIFO control register.
    /// If LCR[7] == 0, this register will be the read-only interrupt IIR register or the
    /// write-only FIFO control register.
    iir_or_fcr: u32,
    /// Line Control Register
    lcr: fields::LineControl,
    /// Modem Control Register
    mcr: u32,
    /// Line Status Register
    lsr: fields::LineStatus,
    /// Modem Status Register
    msr: u32,
    /// Scratch Register
    scr: u32,
}
