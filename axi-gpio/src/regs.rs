//! # Register definitions.

/// Register fields.
pub mod fields {

    /// Data register.
    #[bitbybit::bitfield(u32, debug, defmt_bitfields(feature = "defmt"))]
    pub struct Pins {
        /// Individual data bits.
        #[bit(0, rw)]
        pins: [bool; 32],
    }

    /// Global interrupt enable field.
    #[bitbybit::bitfield(u32, debug, default = 0x0, defmt_bitfields(feature = "defmt"))]
    pub struct GlobalInterruptEnable {
        /// Enable bit.
        #[bit(31, rw)]
        enable: bool,
    }

    /// Interrupt bits field.
    #[bitbybit::bitfield(u32, debug, default = 0x0, defmt_bitfields(feature = "defmt"))]
    pub struct InterruptBits {
        /// Channel 2 bit.
        #[bit(1, rw)]
        channel2: bool,
        /// Channel 1 bit.
        #[bit(0, rw)]
        channel1: bool,
    }
}

/// Register block.
#[derive(derive_mmio::Mmio)]
#[repr(C)]
pub struct Registers {
    ch1_data: fields::Pins,
    ch1_tri_state: fields::Pins,
    ch2_data: fields::Pins,
    ch2_tri_state: fields::Pins,

    _gap0: [u32; 0x43],

    /// Global interrupt enable bit.
    global_interrupt_enable: fields::GlobalInterruptEnable,
    /// Interrupt status bits. This is a Read/Toggle-on-write register.
    interrupt_status: fields::InterruptBits,
    _gap1: u32,
    /// Enable interrupts for individual channels.
    interrupt_enable: fields::InterruptBits,
}

static_assertions::const_assert_eq!(core::mem::size_of::<Registers>(), 0x12C);
