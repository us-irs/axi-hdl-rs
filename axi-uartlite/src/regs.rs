//! # Raw register module

/// Register field types.
pub mod fields {
    /// RX FIFO register.
    #[bitbybit::bitfield(u32, defmt_bitfields(feature = "defmt"))]
    pub struct RxFifo {
        /// Data which can be read.
        #[bits(0..=7, r)]
        pub data: u8,
    }

    /// TX FIFO register.
    #[bitbybit::bitfield(u32, defmt_bitfields(feature = "defmt"))]
    pub struct TxFifo {
        /// Data to be transmitted.
        #[bits(0..=7, w)]
        pub data: u8,
    }

    /// Status register.
    #[bitbybit::bitfield(u32, defmt_bitfields(feature = "defmt"))]
    pub struct Status {
        /// Parity error bit.
        #[bit(7, r)]
        pub parity_error: bool,
        /// Frame error bit.
        #[bit(6, r)]
        pub frame_error: bool,
        /// Overrun error bit.
        #[bit(5, r)]
        pub overrun_error: bool,
        /// Interrupt enabled bit.
        #[bit(4, r)]
        pub intr_enabled: bool,
        /// TX FIFO full.
        #[bit(3, r)]
        pub tx_fifo_full: bool,
        /// TX FIFO empty.
        #[bit(2, r)]
        pub tx_fifo_empty: bool,
        /// RX FIFO full.
        #[bit(1, r)]
        pub rx_fifo_full: bool,
        /// RX FIFO contains valid data.
        #[bit(0, r)]
        pub rx_fifo_valid_data: bool,
    }

    /// Control register.
    #[bitbybit::bitfield(u32, default = 0x0, defmt_bitfields(feature = "defmt"))]
    pub struct Control {
        /// Enable interrupt bit.
        #[bit(4, w)]
        enable_interrupt: bool,
        /// Reset RX FIFO.
        #[bit(1, w)]
        reset_rx_fifo: bool,
        /// Reset TX FIFO.
        #[bit(0, w)]
        reset_tx_fifo: bool,
    }
}

/// AXI UARTLITE register block definition.
#[derive(derive_mmio::Mmio)]
#[repr(C)]
pub struct Registers {
    #[mmio(PureRead)]
    rx_fifo: fields::RxFifo,
    tx_fifo: fields::TxFifo,
    #[mmio(PureRead)]
    stat_reg: fields::Status,
    ctrl_reg: fields::Control,
}
