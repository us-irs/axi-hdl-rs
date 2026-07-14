pub use crate::regs::fields::{Control, Status};
pub use fields::*;

/// Memory-Mapped to AXI-Stream (MM2S) subregister block.
#[derive(derive_mmio::Mmio)]
#[repr(C)]
pub struct Mm2sRegisters {
    control: Control,
    status: Status,
    _gap0: [u32; 0x4],
    source_address_lower_word: u32,
    source_address_upper_word: u32,
    _gap1: [u32; 0x2],
    transfer_length: fields::TransferLength,
}

static_assertions::const_assert_eq!(core::mem::size_of::<Mm2sRegisters>(), 0x2C);

/// AXI-Stream to Memory-Mapped (S2MM) subregister block.
#[derive(derive_mmio::Mmio)]
#[repr(C)]
pub struct S2mmRegisters {
    control: Control,
    status: Status,
    _gap0: [u32; 0x4],
    dest_address_lower_word: u32,
    dest_address_upper_word: u32,
    _gap1: [u32; 0x2],
    transfer_length: fields::TransferLength,
}

static_assertions::const_assert_eq!(core::mem::size_of::<S2mmRegisters>(), 0x2C);

/// Unified register block of the AXI DMA IP core.
///
/// The MM2S and S2MM channels are modelled as inner MMIO blocks so that independent,
/// non-overlapping handles to each channel can be `steal`-ed out of a single [`Registers`]
/// instance.
#[derive(derive_mmio::Mmio)]
#[repr(C)]
pub struct Registers {
    #[mmio(Inner)]
    mm2s: Mm2sRegisters,
    _gap2: u32,
    #[mmio(Inner)]
    s2mm: S2mmRegisters,
}

static_assertions::const_assert_eq!(core::mem::size_of::<Registers>(), 0x5C);

/// Bitfield types specific to direct-register mode.
pub mod fields {
    use arbitrary_int::u26;

    /// Shared MM2S/S2MM transfer length field. Bits 26..=31 are reserved.
    #[bitbybit::bitfield(
        u32,
        default = 0,
        debug,
        defmt_bitfields(feature = "defmt"),
        forbid_overlaps
    )]
    pub struct TransferLength {
        /// Number of bytes to transfer (MM2S) or the maximum to receive (S2MM).
        #[bits(0..=25, rw)]
        length: u26,
    }

    impl From<TransferLength> for u32 {
        #[inline]
        fn from(value: TransferLength) -> Self {
            value.raw_value()
        }
    }
}
