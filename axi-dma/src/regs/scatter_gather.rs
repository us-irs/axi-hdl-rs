use core::{cell::UnsafeCell, mem::MaybeUninit};

use vcell::VolatileCell;

pub use crate::regs::fields::{Control, Status};

/// Scatter-gather mode register block of the AXI DMA IP core.
#[derive(derive_mmio::Mmio)]
#[repr(C)]
pub struct Registers {
    mm2s_control: Control,
    mm2s_status: Status,
    /// The lower 6 bits are reserved and ignored for writes. The address must be 16 word
    /// aligned (e.g. 0x40, 0x80).
    mm2s_current_descriptor_pointer_lower_word: u32,
    mm2s_current_descriptor_pointer_upper_word: u32,
    mm2s_tail_descriptor_lower_word: u32,
    mm2s_tail_descriptor_upper_word: u32,

    _gap0: [u32; 0x5],

    scatter_gather_control: u32,
    s2mm_control: Control,
    s2mm_status: Status,
    /// The lower 6 bits are reserved and ignored for writes. The address must be 16 word
    /// aligned (e.g. 0x40, 0x80).
    s2mm_current_descriptor_pointer_lower_word: u32,
    s2mm_current_descriptor_pointer_upper_word: u32,
    s2mm_tail_descriptor_lower_word: u32,
    s2mm_tail_descriptor_upper_word: u32,
}

static_assertions::const_assert_eq!(core::mem::size_of::<Registers>(), 0x48);

/// Bitfield types specific to scatter-gather mode.
pub mod fields {
    use arbitrary_int::{u4, u26};

    /// Scatter-gather engine control register (SGCTL).
    #[bitbybit::bitfield(
        u32,
        default = 0,
        debug,
        defmt_bitfields(feature = "defmt"),
        forbid_overlaps
    )]
    pub struct SgControl {
        /// AXI `AUSER` sideband value used for descriptor fetches/updates.
        #[bits(8..=11, rw)]
        user: u4,
        /// AXI `ACACHE` value used for descriptor fetches/updates.
        #[bits(0..=3, rw)]
        cache: u4,
    }

    /// Per-[`Descriptor`](super::Descriptor) control word.
    #[bitbybit::bitfield(
        u32,
        default = 0,
        debug,
        defmt_bitfields(feature = "defmt"),
        forbid_overlaps
    )]
    pub struct DescriptorControl {
        /// Should be set by the CPU to indicate that this descriptor describes the start of the
        /// packet.
        #[bit(27, rw)]
        tx_start_of_frame: bool,
        /// Should be set by the CPU to indicate that this descriptor describes the end of the
        /// packet.
        #[bit(26, rw)]
        tx_end_of_frame: bool,
        /// Number of bytes to transfer (MM2S) or the maximum to receive (S2MM).
        #[bits(0..=25, rw)]
        buffer_length: u26,
    }

    /// Per-[`Descriptor`](super::Descriptor) status word, written back by the engine.
    #[bitbybit::bitfield(
        u32,
        default = 0,
        debug,
        defmt_bitfields(feature = "defmt"),
        forbid_overlaps
    )]
    pub struct DescriptorStatus {
        /// Set by the engine once this descriptor's transfer has completed.
        #[bit(31, rw)]
        completed: bool,
        /// DMA/data stream decode error.
        #[bit(30, rw)]
        dma_decode_error: bool,
        /// DMA slave (AXI) error.
        #[bit(29, rw)]
        dma_slave_error: bool,
        /// DMA internal error.
        #[bit(28, rw)]
        dma_internal_error: bool,
        /// Number of bytes actually transferred.
        #[bits(0..=25, rw)]
        transferred_bytes: u26,
    }
}

/// A single scatter-gather buffer descriptor, linked into a ring via
/// `next_descriptor_pointer_*`.
#[repr(C, align(0x40))]
pub struct Descriptor {
    /// The lower 6 bits are reserved and ignored for writes. The address must be 16 word
    /// aligned (e.g. 0x40, 0x80).
    next_descriptor_pointer_lower_word: VolatileCell<u32>,
    next_descriptor_pointer_upper_word: VolatileCell<u32>,
    buffer_address_lower_word: VolatileCell<u32>,
    buffer_address_upper_word: VolatileCell<u32>,
    _reserved: [VolatileCell<u32>; 2],
    control: VolatileCell<fields::DescriptorControl>,
    status: VolatileCell<fields::DescriptorStatus>,
    app_words: [VolatileCell<u32>; 5],
}

impl Descriptor {
    /// Creates a zeroed descriptor.
    #[inline]
    pub const fn new() -> Self {
        Self {
            next_descriptor_pointer_lower_word: VolatileCell::new(0),
            next_descriptor_pointer_upper_word: VolatileCell::new(0),
            buffer_address_lower_word: VolatileCell::new(0),
            buffer_address_upper_word: VolatileCell::new(0),
            _reserved: [const { VolatileCell::new(0) }; 2],
            control: VolatileCell::new(fields::DescriptorControl::new_with_raw_value(0)),
            status: VolatileCell::new(fields::DescriptorStatus::new_with_raw_value(0)),
            app_words: [const { VolatileCell::new(0) }; 5],
        }
    }

    /// Reads back this descriptor's status word.
    #[inline]
    pub fn status_word(&self) -> fields::DescriptorStatus {
        self.status.get()
    }

    /// Sets this descriptor's control word (buffer length, start-/end-of-frame flags).
    #[inline]
    pub fn set_control(&self, control: fields::DescriptorControl) {
        self.control.set(control);
    }

    /// Reads back this descriptor's control word.
    #[inline]
    pub fn control_word(&self) -> fields::DescriptorControl {
        self.control.get()
    }

    /// Sets the buffer address this descriptor points to: the source buffer for MM2S, or the
    /// destination buffer for S2MM.
    #[inline]
    pub fn set_buffer_address(&self, addr: usize) {
        let addr = addr as u64;
        self.buffer_address_lower_word.set(addr as u32);
        self.buffer_address_upper_word.set((addr >> 32) as u32);
    }
}

impl Default for Descriptor {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

/// This is a low level wrapper to simplify declaring a global descriptor list.
///
/// It allows placing the descriptor structure statically in memory which might not
/// be zero-initialized.
#[repr(transparent)]
pub struct DescriptorList<const SLOTS: usize>(pub UnsafeCell<MaybeUninit<[Descriptor; SLOTS]>>);

unsafe impl<const SLOTS: usize> Sync for DescriptorList<SLOTS> {}

impl<const SLOTS: usize> DescriptorList<SLOTS> {
    /// Creates an uninitialized descriptor list. Call [`Self::take`] to initialize and use it.
    #[inline]
    pub const fn new() -> Self {
        Self(UnsafeCell::new(MaybeUninit::uninit()))
    }

    /// Initializes the RX descriptors and returns a mutable reference to them.
    ///
    /// Requires `&'static self` (i.e. `self` must actually be a `static`, not a local/stack
    /// value) since the returned reference borrows the same memory for `'static`; the compiler
    /// enforces that placement requirement, so it isn't a safety precondition below.
    ///
    /// # Safety
    ///
    /// This allows creating aliasing mutable references and circumventing ownership and safety
    /// guarantees of the HAL. You MUST call this function only once per descriptor instance.
    // The `&'static self -> &'static mut` shape is exactly what triggers this lint, but it's the
    // intended pattern here: `self.0` is an `UnsafeCell`, so the shared `&self` doesn't actually
    // alias the `&mut` this hands out; the real aliasing hazard is documented above instead.
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn take(&'static self) -> &'static mut [Descriptor; SLOTS] {
        let descr = unsafe { &mut *self.0.get() };
        descr.write([const { Descriptor::new() }; SLOTS]);
        unsafe { descr.assume_init_mut() }
    }
}

impl<const SLOTS: usize> Default for DescriptorList<SLOTS> {
    fn default() -> Self {
        Self::new()
    }
}

/// Configures a descriptor list cyclic by linking each descriptor's `next_descriptor_pointer` to
/// its successor, wrapping the last descriptor's back around to the first, so the DMA engine
/// walks the whole ring and loops forever instead of running off the end after the first
/// descriptor (whose `next_descriptor_pointer` would otherwise stay null).
pub fn configure_descriptors_cyclic(descriptors: &mut [Descriptor]) {
    let len = descriptors.len();
    if len == 0 {
        return;
    }
    let base_addr = descriptors.as_ptr() as usize;
    let stride = core::mem::size_of::<Descriptor>();
    for (i, descriptor) in descriptors.iter_mut().enumerate() {
        // Widening to u64 (rather than branching on `size_of::<usize>()`) keeps this correct
        // regardless of target pointer width instead of silently doing nothing on one we didn't
        // anticipate.
        let next_addr = (base_addr + ((i + 1) % len) * stride) as u64;
        descriptor
            .next_descriptor_pointer_lower_word
            .set(next_addr as u32);
        descriptor
            .next_descriptor_pointer_upper_word
            .set((next_addr >> 32) as u32);
    }
}
