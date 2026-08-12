//! # Driver for the Xilinx/AMD AXI DMA IP core (PG021)
//!
//! Supports both simple MM2S/S2MM transfers, and asynchronous interrupt-driven transfers.
//!
//! ## Cache maintenance
//!
//! This crate is platform-agnostic: it only performs volatile MMIO register accesses and has no
//! knowledge of the CPU's cache hierarchy. The AXI DMA engine might read and write physical memory
//! directly, bypassing any CPU data cache. If your target has a data cache, you are
//! responsible for explicit cache maintenance around every transfer.
//!
//! - Before starting an MM2S (write/TX) transfer, clean the source buffer's cache lines (push
//!   any dirty CPU writes out to memory), so the DMA engine reads what you actually wrote rather
//!   than stale memory content.
//! - After an S2MM (read/RX) transfer completes, invalidate the destination buffer's cache
//!   lines, so the next CPU read is served from memory (what the DMA engine actually wrote)
//!   instead of a stale cached copy. The DMA engine might write straight to physical memory
//!   regardless of cache state, so nothing needs to happen to the
//!   destination buffer's cache lines *before* arming the transfer, only after it completes and
//!   before you read the result.
//!
//! Skipping this is a common source of "DMA always reads zeros" or "DMA sends garbage" bugs that
//! only show up on real hardware. Which exact operations are needed, and whether they're needed
//! at all, is platform- and even buffer-placement-specific: it depends on your SoC's cache
//! architecture and how the target memory region happens to be mapped by the MMU. Some
//! platforms let you place DMA buffers in genuinely non-cacheable memory, which removes the
//! need for maintenance entirely, but that has to be set up outside this crate, and even then
//! placement alone doesn't always mean what you'd expect: see `zynq7000_hal::cache` and
//! `zynq7000-rt`'s OCM documentation for a concrete example of a memory region that is still
//! cacheable despite being on-chip.
//!
//! Consult your platform HAL's cache module for the actual maintenance primitives and how they
//! apply to your buffers.
//!
//! ## Cargo features
//!
//! - `1-waker`, `2-wakers`, `4-wakers`, `8-wakers`, `16-wakers`, `32-wakers` select
//!   [`NUM_TX_WAKERS`], the size of the global waker table backing [`DmaWriterAsync`]. Only one of
//!   these can be active at a time. Each concurrently-live async writer (each `tx_waker_index`
//!   passed to [`DmaController::take_writer_async`]) needs its own slot, so this bounds how many
//!   independent async MM2S channels can be in flight across all [`DmaController`] instances at
//!   once, not just on a single controller. `1-waker` is the default and covers the common case
//!   of a single AXI DMA peripheral doing one async transfer at a time. Pick a larger option if
//!   you have multiple AXI DMA peripherals, or otherwise need more than one waker slot claimed at
//!   once. This can't be changed at runtime, so you have to pick a build-time upper bound.
//! - `portable-atomic` switches every atomic type this crate uses (`AtomicBool`, `AtomicPtr`,
//!   `AtomicU8`, `AtomicUsize`) from `core::sync::atomic` to the [`portable-atomic`
//!   crate](https://docs.rs/portable-atomic)'s equivalents. This is an AMD/Xilinx HDL IP core, so
//!   it only ever runs on AMD/Xilinx SoC or FPGA targets: Zynq-7000/UltraScale+'s Cortex-A/R
//!   cores, or the MicroBlaze soft core. Enable this if your particular target or configuration
//!   (for example a MicroBlaze build without an atomic exchange instruction) doesn't provide
//!   native atomics of the required width. `portable-atomic` polyfills them, typically through a
//!   critical section. Off by default, since the common Cortex-A/R targets already have native
//!   atomic support.
//! - `defmt` implements [`defmt::Format`](https://docs.rs/defmt) for this crate's register
//!   bitfield and error types, so they can be logged directly with `defmt::info!`/`defmt::error!`
//!   and similar, instead of only `core::fmt::Debug`. Off by default to avoid pulling in `defmt`
//!   for users who don't use it.
#![no_std]
#![deny(missing_docs)]

use core::{future::poll_fn, sync::atomic::Ordering};

use arbitrary_int::{traits::Integer as _, u26};
#[cfg(not(feature = "portable-atomic"))]
use core::sync::atomic::{AtomicBool, AtomicPtr, AtomicU8, AtomicUsize};
use embassy_sync::waitqueue::AtomicWaker;
#[cfg(feature = "portable-atomic")]
use portable_atomic::{AtomicBool, AtomicPtr, AtomicU8, AtomicUsize};

use crate::regs::{direct_register::TransferLength, fields::Control};
pub mod regs;

/// 1 waker (default).
#[cfg(feature = "1-waker")]
pub const NUM_TX_WAKERS: usize = 1;
/// 2 wakers
#[cfg(feature = "2-wakers")]
pub const NUM_TX_WAKERS: usize = 2;
/// 4 wakers
#[cfg(feature = "4-wakers")]
pub const NUM_TX_WAKERS: usize = 4;
/// 8 wakers
#[cfg(feature = "8-wakers")]
pub const NUM_TX_WAKERS: usize = 8;
/// 16 wakers
#[cfg(feature = "16-wakers")]
pub const NUM_TX_WAKERS: usize = 16;
/// 32 wakers
#[cfg(feature = "32-wakers")]
pub const NUM_TX_WAKERS: usize = 32;

static TX_WAKERS: [AtomicWaker; NUM_TX_WAKERS] = [const { AtomicWaker::new() }; NUM_TX_WAKERS];
static TX_TRANSFER_DONE: [AtomicBool; NUM_TX_WAKERS] =
    [const { AtomicBool::new(false) }; NUM_TX_WAKERS];
/// Per-slot error outcome, set alongside `TX_TRANSFER_DONE` by [`DmaWriterAsync::on_interrupt`]
/// so the task woken by that flag can tell a failed transfer from a completed one and recover the
/// [`DmaTransferError`] detail; see [`DmaTransferError::to_bits`]/[`DmaTransferError::from_bits`].
static TX_TRANSFER_ERROR: [AtomicU8; NUM_TX_WAKERS] = [const { AtomicU8::new(0) }; NUM_TX_WAKERS];
/// Global ownership table for waker slots, shared by every [`DmaController`] instance, since
/// `TX_WAKERS`/`TX_TRANSFER_DONE` are global too. Claimed atomically via [`claim_tx_waker`].
static TX_WAKER_TAKEN: [AtomicBool; NUM_TX_WAKERS] =
    [const { AtomicBool::new(false) }; NUM_TX_WAKERS];

/// Number of times [`DmaController::new`] polls a channel's `reset` bit before giving up.
///
/// This crate has no timer or delay dependency, so this is a poll count rather than an actual
/// time-based timeout. It just needs to be generous enough that it never trips on real hardware,
/// while still keeping `new()` from hanging forever if `base_addr` doesn't point at a real AXI
/// DMA peripheral (an unmapped or otherwise bogus address can read back a `reset` bit that never
/// clears).
const RESET_POLL_ATTEMPTS: u32 = 100_000;

/// A channel's `reset` bit did not clear within the poll timeout. This usually means `base_addr`
/// does not point at a real AXI DMA peripheral.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("DMA channel reset did not complete within the timeout")]
pub struct ResetTimeoutError;

/// Polls `is_resetting` up to [`RESET_POLL_ATTEMPTS`] times, returning once it reports `false`.
fn wait_for_reset(mut is_resetting: impl FnMut() -> bool) -> Result<(), ResetTimeoutError> {
    for _ in 0..RESET_POLL_ATTEMPTS {
        if !is_resetting() {
            return Ok(());
        }
    }
    Err(ResetTimeoutError)
}

/// A requested DMA transfer buffer is longer than the 2^26 - 1 byte limit the transfer-length
/// register can encode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error(
    "invalid DMA transfer buffer length {len}: must be less than or equal to {} bytes",
    u26::MAX.as_usize()
)]
pub struct InvalidBufferLengthError {
    /// The buffer length that was rejected.
    pub len: usize,
}

/// `tx_waker_index` is out of range for the crate's configured [`NUM_TX_WAKERS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("invalid waker slot index: {0}")]
pub struct InvalidTxWakerIndexError(pub usize);

/// `tx_waker_index` was already claimed by another async writer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("waker slot index {0} is already in use by another async writer")]
pub struct TxWakerIndexInUseError(pub usize);

/// Error returned when claiming a waker slot for an async writer fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TakeAsyncError {
    /// See [`InvalidTxWakerIndexError`].
    #[error(transparent)]
    InvalidWakerIndex(#[from] InvalidTxWakerIndexError),
    /// See [`TxWakerIndexInUseError`].
    #[error(transparent)]
    WakerIndexInUse(#[from] TxWakerIndexInUseError),
}

/// Reports the underlying DMA/SG error bits latched alongside a channel's `error_interrupt`
/// status bit. Returned by `on_interrupt` instead of being silently cleared, so the caller finds
/// out that something actually went wrong on the bus rather than the transfer just completing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("DMA transfer error (decode: {decode}, slave: {slave}, internal: {internal})")]
pub struct DmaTransferError {
    /// DMA/data stream decode error.
    pub decode: bool,
    /// DMA slave (AXI) error.
    pub slave: bool,
    /// DMA internal error.
    pub internal: bool,
}

impl DmaTransferError {
    const PRESENT_BIT: u8 = 0b1000;

    /// Packs this error into a single byte for storage in [`TX_TRANSFER_ERROR`], which needs a
    /// lock-free `Copy` representation to hand an error from the interrupt handler to the task
    /// woken by the matching [`TX_TRANSFER_DONE`] slot.
    #[inline]
    const fn to_bits(self) -> u8 {
        Self::PRESENT_BIT
            | (self.decode as u8)
            | (self.slave as u8) << 1
            | (self.internal as u8) << 2
    }

    /// Inverse of [`Self::to_bits`]. Returns `None` for `0`, the initial/no-error state of
    /// [`TX_TRANSFER_ERROR`]'s slots.
    #[inline]
    const fn from_bits(bits: u8) -> Option<Self> {
        if bits & Self::PRESENT_BIT == 0 {
            return None;
        }
        Some(Self {
            decode: bits & 0b001 != 0,
            slave: bits & 0b010 != 0,
            internal: bits & 0b100 != 0,
        })
    }
}

/// Error returned by [`DmaWriterAsync::write`]: either the buffer was rejected up front, or the
/// transfer was armed but the hardware reported an error on completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DmaWriteError {
    /// See [`InvalidBufferLengthError`].
    #[error(transparent)]
    InvalidBufferLength(#[from] InvalidBufferLengthError),
    /// See [`DmaTransferError`].
    #[error(transparent)]
    Transfer(#[from] DmaTransferError),
}

impl embedded_io_async::Error for DmaWriteError {
    fn kind(&self) -> embedded_io::ErrorKind {
        embedded_io::ErrorKind::Other
    }
}

/// Top-level driver for an AXI DMA peripheral instance, used to hand out the individual
/// reader/writer handles.
pub struct DmaController {
    regs: regs::direct_register::MmioRegisters<'static>,
    writer_taken: bool,
    reader_taken: bool,
}

impl DmaController {
    /// Create a new AXI DMA controller peripheral driver.
    ///
    /// # Safety
    ///
    /// - The `base_addr` must be a valid memory-mapped register address of an AXI DMA peripheral.
    /// - Dereferencing an invalid or misaligned address results in **undefined behavior**.
    /// - The caller must ensure that no other code concurrently modifies the same peripheral registers
    ///   in an unsynchronized manner to prevent data races.
    /// - This function does not enforce uniqueness of driver instances. Creating multiple instances
    ///   with the same `base_addr` can lead to unintended behavior if not externally synchronized.
    /// - The driver performs **volatile** reads and writes to the provided address.
    pub fn new(base_addr: usize) -> Result<Self, ResetTimeoutError> {
        let mut regs = unsafe { regs::direct_register::Registers::new_mmio_at(base_addr) };
        regs.mm2s().write_control(Control::ZERO.with_reset(true));
        regs.s2mm().write_control(Control::ZERO.with_reset(true));
        wait_for_reset(|| regs.mm2s().read_control().reset())?;
        wait_for_reset(|| regs.s2mm().read_control().reset())?;
        Ok(Self {
            regs,
            writer_taken: false,
            reader_taken: false,
        })
    }

    /// This API can be used to retrieve a simple DMA writer once.
    pub fn take_simple_writer(&mut self) -> Option<SimpleDmaWriter> {
        if self.writer_taken {
            return None;
        }
        self.writer_taken = true;
        // Safety: Ownership check done above.
        Some(unsafe { self.steal_simple_writer() })
    }

    /// Steal a simple writer instance
    ///
    /// # Safety
    ///
    /// Allows creating multiple handles to the same peripheral, which can lead to data races.
    pub unsafe fn steal_simple_writer(&mut self) -> SimpleDmaWriter {
        SimpleDmaWriter {
            regs: unsafe { self.regs.steal_mm2s() },
        }
    }

    /// This API can be used to retrieve an async DMA writer once.
    pub fn take_writer_async(
        &mut self,
        tx_waker_index: usize,
    ) -> Result<Option<DmaWriterAsync>, TakeAsyncError> {
        if self.writer_taken {
            return Ok(None);
        }
        // Safety: writer ownership checked above; steal_writer_async claims the waker
        // slot (or fails) before we commit to writer_taken below.
        let writer = unsafe { self.steal_writer_async(tx_waker_index)? };
        self.writer_taken = true;
        Ok(Some(writer))
    }

    /// Steal an async simple writer instance.
    ///
    /// # Safety
    ///
    /// Allows creating multiple handles to the same peripheral, which can lead to data races.
    pub unsafe fn steal_writer_async(
        &mut self,
        tx_waker_index: usize,
    ) -> Result<DmaWriterAsync, TakeAsyncError> {
        claim_tx_waker(tx_waker_index)?;
        let regs = unsafe { self.regs.steal_mm2s() };
        let token = DmaTxToken {
            // SAFETY: Only converted to primitive address
            base_addr: unsafe { regs.ptr() } as usize,
            tx_waker_index,
        };
        Ok(DmaWriterAsync { regs, token })
    }

    /// This API can be used to retrieve a simple DMA reader once.
    pub fn take_simple_reader(&mut self) -> Option<SimpleDmaReader> {
        if self.reader_taken {
            return None;
        }
        self.reader_taken = true;
        // Safety: Ownership check done above.
        Some(unsafe { self.steal_simple_reader() })
    }

    /// Steal a simple reader instance
    ///
    /// # Safety
    ///
    /// Allows creating multiple handles to the same peripheral, which can lead to data races.
    pub unsafe fn steal_simple_reader(&mut self) -> SimpleDmaReader {
        SimpleDmaReader {
            regs: unsafe { self.regs.steal_s2mm() },
        }
    }

    /// This API can be used to retrieve an ISR-driven DMA reader once.
    pub fn take_reader_interrupt_driven(&mut self) -> Option<DmaReaderInterruptDriven> {
        if self.reader_taken {
            return None;
        }
        self.reader_taken = true;
        // Safety: Ownership check done above.
        Some(unsafe { self.steal_reader_interrupt_driven() })
    }

    /// Steal an ISR-driven simple reader instance.
    ///
    /// # Safety
    ///
    /// Allows creating multiple handles to the same peripheral, which can lead to data races.
    pub unsafe fn steal_reader_interrupt_driven(&mut self) -> DmaReaderInterruptDriven {
        let regs = unsafe { self.regs.steal_s2mm() };
        let token = DmaInterruptRxToken {
            // SAFETY: Only converted to primitive address
            base_addr: unsafe { regs.ptr() } as usize,
        };
        DmaReaderInterruptDriven { regs, token }
    }
}

/// Atomically claims `tx_waker_index` in the global [`TX_WAKER_TAKEN`] table.
fn claim_tx_waker(tx_waker_index: usize) -> Result<(), TakeAsyncError> {
    if tx_waker_index >= NUM_TX_WAKERS {
        return Err(InvalidTxWakerIndexError(tx_waker_index).into());
    }
    TX_WAKER_TAKEN[tx_waker_index]
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| TxWakerIndexInUseError(tx_waker_index))?;
    Ok(())
}

/// A blocking, polling-driven MM2S (write/TX) DMA writer.
#[derive(Debug)]
pub struct SimpleDmaWriter {
    regs: regs::direct_register::MmioMm2sRegisters<'static>,
}

/// An armed, in-progress MM2S transfer started by [`SimpleDmaWriter::start_write`].
///
/// Borrows both the writer and its source buffer for as long as the transfer might still be
/// running, so the buffer can't be touched or dropped out from under the DMA engine; this is
/// sound because completion is only ever observed synchronously, on the same call stack that
/// armed the transfer, via [`Self::poll`].
pub struct SimpleDmaWriteTransfer<'a> {
    regs: &'a mut regs::direct_register::MmioMm2sRegisters<'static>,
    // Not read again here, but the transfer borrows the source buffer for as long as the DMA
    // engine might still be reading from it; see the struct-level safety note.
    _buf: &'a [u8],
}

impl SimpleDmaWriteTransfer<'_> {
    /// Non-blocking check for completion. Returns `true` once the transfer has completed.
    #[inline]
    pub fn poll(&mut self) -> bool {
        self.regs.read_status().idle()
    }

    /// Blocks until the transfer completes.
    pub fn wait(mut self) {
        while !self.poll() {}
    }
}

impl SimpleDmaWriter {
    /// Programs the registers to start an MM2S transfer of `buf` and returns immediately; use
    /// the returned [`SimpleDmaWriteTransfer`] to find out when it's done.
    ///
    /// Pleaes note that the source address must be aligned to the MM2S memory map data width
    /// if the data realignment engine is not included.
    pub fn start_write<'a>(
        &'a mut self,
        buf: &'a [u8],
    ) -> Result<SimpleDmaWriteTransfer<'a>, InvalidBufferLengthError> {
        if buf.len() > u26::MAX.as_usize() {
            return Err(InvalidBufferLengthError { len: buf.len() });
        }
        start_write(&mut self.regs, buf)?;
        Ok(SimpleDmaWriteTransfer {
            regs: &mut self.regs,
            _buf: buf,
        })
    }

    /// Blocking write function using DMA. Equivalent to [`Self::start_write`] immediately
    /// followed by [`SimpleDmaWriteTransfer::wait`].
    #[inline]
    pub fn write(&mut self, buf: &[u8]) -> Result<(), InvalidBufferLengthError> {
        self.start_write(buf)?.wait();
        Ok(())
    }
}

impl embedded_io_async::ErrorType for SimpleDmaWriter {
    type Error = InvalidBufferLengthError;
}

impl embedded_io::Write for SimpleDmaWriter {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.write(buf)?;
        Ok(buf.len())
    }

    #[inline]
    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// Interrupt-driven, async MM2S (write/TX) DMA writer.
#[derive(Debug)]
pub struct DmaWriterAsync {
    regs: regs::direct_register::MmioMm2sRegisters<'static>,
    token: DmaTxToken,
}

/// Identifies a [`DmaWriterAsync`]'s MM2S channel, e.g. for use in an interrupt handler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DmaTxToken {
    base_addr: usize,
    tx_waker_index: usize,
}

impl DmaTxToken {
    /// The MM2S register block's base address.
    #[inline]
    pub fn base_addr(&self) -> usize {
        self.base_addr
    }

    /// The waker slot this token's writer was constructed with.
    #[inline]
    pub fn tx_waker_index(&self) -> usize {
        self.tx_waker_index
    }

    /// Constructs a transfer token from a raw base address and waker index, e.g. for use in an
    /// interrupt handler that only has these two values available from static configuration,
    /// rather than a token returned by a previous [`DmaWriterAsync::write`] call.
    ///
    /// # Safety
    ///
    /// The caller must ensure `base_addr` is the real base address of the MM2S register block
    /// whose completion interrupt is being serviced, and that `tx_waker_index` matches the slot
    /// originally passed to the corresponding `take_writer_async` call.
    #[inline]
    pub unsafe fn steal(tx_waker_index: usize, base_addr: usize) -> Self {
        Self {
            base_addr,
            tx_waker_index,
        }
    }
}

impl DmaWriterAsync {
    /// The token identifying this writer's MM2S channel, fixed for the writer's whole lifetime.
    /// Retrieve it once, right after construction, to hand to your interrupt handler — no need
    /// to wait for a `write` call to return one.
    #[inline]
    pub fn token(&self) -> DmaTxToken {
        self.token
    }

    /// Arms an MM2S transfer of `buf` and awaits its completion (or error), driven by
    /// [`Self::on_interrupt`] called from your interrupt handler.
    pub async fn write(&mut self, buf: &[u8]) -> Result<(), DmaWriteError> {
        let token = self.token;
        TX_TRANSFER_DONE[token.tx_waker_index].store(false, Ordering::Relaxed);
        TX_TRANSFER_ERROR[token.tx_waker_index].store(0, Ordering::Relaxed);
        // Enable relevant interrupts.
        self.regs.modify_control(|val| {
            val.with_interrupt_on_complete(true)
                .with_error_interrupt_enable(true)
        });
        // Clear any pending interrupt bits which might still be set.
        self.regs.write_status(
            regs::fields::Status::ZERO
                .with_completion_interrupt(true)
                .with_delay_timer_interrupt(true)
                .with_error_interrupt(true),
        );
        start_write(&mut self.regs, buf)?;
        poll_fn(move |cx| {
            TX_WAKERS[token.tx_waker_index].register(cx.waker());

            if TX_TRANSFER_DONE[token.tx_waker_index].load(Ordering::Relaxed) {
                return core::task::Poll::Ready(());
            }
            core::task::Poll::Pending
        })
        .await;
        // `on_interrupt` stores the error (if any) before marking the slot done, so it's always
        // visible here once the transfer has settled.
        if let Some(error) = DmaTransferError::from_bits(
            TX_TRANSFER_ERROR[token.tx_waker_index].load(Ordering::Relaxed),
        ) {
            return Err(error.into());
        }
        Ok(())
    }

    /// Like [`Self::write`], but retries until all of `buf` has been written.
    #[inline]
    pub async fn write_all(&mut self, buf: &[u8]) -> Result<(), DmaWriteError> {
        <Self as embedded_io_async::Write>::write_all(self, buf).await?;
        Ok(())
    }

    /// Services the MM2S completion interrupt for the transfer identified by `token`. Call this
    /// from your interrupt handler.
    ///
    /// Returns `Some(error)` if the `error_interrupt` status bit was set, i.e. the transfer
    /// didn't complete cleanly. In that case the task awaiting [`Self::write`] is woken the same
    /// as on a normal completion, but observes an error result from [`Self::write`] instead of
    /// success — this return value is only for a caller polling `on_interrupt` directly (e.g.
    /// logging from the actual interrupt handler); it isn't otherwise propagated anywhere else.
    ///
    /// # Safety
    ///
    /// `token` must have been returned by (or constructed via [`DmaTxToken::steal`]
    /// to match) a transfer actually started on this MM2S channel.
    pub unsafe fn on_interrupt(token: &DmaTxToken) -> Option<DmaTransferError> {
        let mut regs =
            unsafe { regs::direct_register::Mm2sRegisters::new_mmio_at(token.base_addr) };
        let status = regs.read_status();
        let error = if status.error_interrupt() {
            regs.write_status(regs::fields::Status::ZERO.with_error_interrupt(true));
            Some(DmaTransferError {
                decode: status.dma_decode_error(),
                slave: status.dma_slave_error(),
                internal: status.dma_internal_error(),
            })
        } else {
            None
        };
        // A hardware error halts the channel without ever setting `completion_interrupt` (PG021),
        // so the task must be woken here too, or `write()` would hang forever on a transfer that
        // fails rather than completes.
        if status.completion_interrupt() || error.is_some() {
            if let Some(error) = error {
                TX_TRANSFER_ERROR[token.tx_waker_index].store(error.to_bits(), Ordering::Relaxed);
            }
            TX_TRANSFER_DONE[token.tx_waker_index].store(true, Ordering::Relaxed);
            TX_WAKERS[token.tx_waker_index].wake();
        }
        if status.completion_interrupt() {
            regs.write_status(regs::fields::Status::ZERO.with_completion_interrupt(true));
        }
        error
    }
}

impl embedded_io_async::Error for InvalidBufferLengthError {
    #[inline]
    fn kind(&self) -> embedded_io::ErrorKind {
        embedded_io::ErrorKind::Other
    }
}

impl embedded_io_async::ErrorType for DmaWriterAsync {
    type Error = DmaWriteError;
}

impl embedded_io_async::Write for DmaWriterAsync {
    #[inline]
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.write(buf).await?;
        Ok(buf.len())
    }

    #[inline]
    async fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// An armed, in-progress S2MM transfer started by [`SimpleDmaReader::start_read`].
///
/// Borrows both the reader and its destination buffer for as long as the transfer might still
/// be running, so the buffer can't be touched or dropped out from under the DMA engine; this is
/// sound (unlike the ISR-driven [`DmaReaderInterruptDriven`]) because completion is only ever
/// observed synchronously, on the same call stack that armed the transfer, via [`Self::poll`].
pub struct SimpleDmaReadTransfer<'a> {
    regs: &'a mut regs::direct_register::MmioS2mmRegisters<'static>,
    buf: &'a mut [u8],
}

impl SimpleDmaReadTransfer<'_> {
    /// Non-blocking check for completion.
    ///
    /// Returns `None` while the transfer is still in progress. Once done, returns
    /// `Some(data)`, a view into the buffer passed to [`SimpleDmaReader::start_read`] truncated
    /// to the number of bytes actually received — this can be less than the buffer's length,
    /// e.g. on a short frame terminated early via `TLAST`.
    pub fn poll(&mut self) -> Option<&mut [u8]> {
        let len = self.poll_len()?;
        Some(&mut self.buf[..len])
    }

    /// Non-blocking completion check that returns just the received length instead of a slice
    /// borrowed from `buf`, so it can be called repeatedly with an ordinary short-lived
    /// `&mut self` — used by both [`Self::poll`] and [`Self::wait`] to share the single
    /// register read of the length without either of them needing a `'a`-tied borrow in the
    /// polling loop.
    fn poll_len(&mut self) -> Option<usize> {
        if !self.regs.read_status().idle() {
            return None;
        }
        Some(self.regs.read_transfer_length().length().as_usize())
    }
}

impl<'a> SimpleDmaReadTransfer<'a> {
    /// Blocks until the transfer completes and returns the received data. See [`Self::poll`]
    /// for details on the returned slice's length.
    pub fn wait(mut self) -> &'a mut [u8] {
        // Loop on `poll_len` (plain `&mut self`, owned `usize` result) rather than `poll`
        // (which would need to borrow `self` for all of `'a` on every iteration, and so
        // couldn't be called more than once). The single `&'a mut` borrow of `buf` is built
        // exactly once below, after the loop, once no other borrow of `self` is outstanding.
        let len = loop {
            if let Some(len) = self.poll_len() {
                break len;
            }
        };
        &mut self.buf[..len]
    }
}

/// A blocking, polling-driven S2MM (read/RX) DMA reader.
#[derive(Debug)]
pub struct SimpleDmaReader {
    regs: regs::direct_register::MmioS2mmRegisters<'static>,
}

impl SimpleDmaReader {
    /// Programs the registers to start an S2MM transfer into `buf` and returns immediately;
    /// use the returned [`SimpleDmaReadTransfer`] to find out when it's done.
    ///
    /// Please note that the destination address must be aligned to the S2MM memory map data
    /// width if the data realignment engine is not included.
    pub fn start_read<'a>(
        &'a mut self,
        buf: &'a mut [u8],
    ) -> Result<SimpleDmaReadTransfer<'a>, InvalidBufferLengthError> {
        if buf.len() > u26::MAX.as_usize() {
            return Err(InvalidBufferLengthError { len: buf.len() });
        }
        start_read(&mut self.regs, buf)?;
        Ok(SimpleDmaReadTransfer {
            regs: &mut self.regs,
            buf,
        })
    }

    /// Blocking read function using DMA. Equivalent to [`Self::start_read`] immediately
    /// followed by [`SimpleDmaReadTransfer::wait`].
    #[inline]
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, InvalidBufferLengthError> {
        Ok(self.start_read(buf)?.wait().len())
    }
}

impl embedded_io::ErrorType for SimpleDmaReader {
    type Error = InvalidBufferLengthError;
}

impl embedded_io::Read for SimpleDmaReader {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        self.read(buf)
    }
}

/// Identifies an in-flight (or just-completed) S2MM transfer for [`DmaReaderInterruptDriven`], e.g.
/// for use in an interrupt handler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DmaInterruptRxToken {
    base_addr: usize,
}

impl DmaInterruptRxToken {
    /// The S2MM register block's base address.
    #[inline]
    pub fn base_addr(&self) -> usize {
        self.base_addr
    }

    /// Constructs a read token from a raw base address, e.g. for use in an interrupt handler
    /// that only has this value available from static configuration rather than a token
    /// returned by a previous [`DmaReaderInterruptDriven::start`] call.
    ///
    /// # Safety
    ///
    /// The caller must ensure `base_addr` is the real base address of the S2MM register block
    /// whose completion interrupt is being serviced.
    #[inline]
    pub unsafe fn steal(base_addr: usize) -> Self {
        Self { base_addr }
    }
}

/// Error returned by [`DmaReaderInterruptDriven::on_interrupt`].
#[derive(Debug, thiserror::Error)]
pub enum DmaReadError {
    /// See [`DmaTransferError`]. The transfer whose completion interrupt this call is servicing
    /// itself failed on the bus, so no data is available.
    #[error(transparent)]
    Transfer(#[from] DmaTransferError),
    /// The transfer completed, but the buffer `next_buf` returned to arm next could not be
    /// armed (e.g. it was too long), so the channel is now disarmed. You need to call
    /// [`DmaReaderInterruptDriven::start`] again yourself.
    #[error("failed to re-arm the next S2MM transfer: {error}")]
    Rearm {
        /// The data received by the transfer that just completed. Not lost just because
        /// arming the next one failed.
        data: &'static mut [u8],
        /// Why re-arming failed.
        error: InvalidBufferLengthError,
    },
}

/// Ping-pongs [`DmaReaderInterruptDriven`] between a pair of same-length buffers. See
/// [`DmaReaderInterruptDriven::on_interrupt_double_buffered`].
#[derive(Debug)]
pub struct DoubleBufferHelper {
    buffer_0_ptr: AtomicPtr<u8>,
    buffer_1_ptr: AtomicPtr<u8>,
    buffer_len: AtomicUsize,
}

impl Default for DoubleBufferHelper {
    fn default() -> Self {
        Self::new()
    }
}

/// The two buffers passed to [`DoubleBufferHelper::init`] have different lengths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("buffer length is not equal")]
pub struct LengthNotEqualError;

/// Error returned by [`DoubleBufferHelper::init`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DoubleBufferInitError {
    /// See [`LengthNotEqualError`].
    #[error(transparent)]
    LengthNotEqual(#[from] LengthNotEqualError),
    /// [`DoubleBufferHelper::init`] was already called once on this instance.
    #[error("double buffer helper was already initialized")]
    AlreadyInitialized,
}

impl DoubleBufferHelper {
    /// Creates an empty helper with no buffers assigned yet, so it can sit directly in a plain
    /// top-level `static` (its fields are already just atomics, no extra cell needed) and be
    /// referenced by name as many times as needed — e.g. once from your task to call
    /// [`Self::init`] and arm the first transfer, and again from your interrupt handler.
    #[inline]
    pub const fn new() -> Self {
        Self {
            buffer_0_ptr: AtomicPtr::new(core::ptr::null_mut()),
            buffer_1_ptr: AtomicPtr::new(core::ptr::null_mut()),
            buffer_len: AtomicUsize::new(0),
        }
    }

    /// Assigns the pair of same-length buffers to ping-pong between. Only succeeds once per
    /// instance (guarded by an atomic claim on the still-`null` `buffer_0` slot); call it before
    /// arming the first transfer.
    pub fn init(
        &self,
        buf0: &'static mut [u8],
        buf1: &'static mut [u8],
    ) -> Result<(), DoubleBufferInitError> {
        if buf0.len() != buf1.len() {
            return Err(LengthNotEqualError.into());
        }
        // Claims the uninitialized (null) state; fails if some previous call already did.
        self.buffer_0_ptr
            .compare_exchange(
                core::ptr::null_mut(),
                buf0.as_mut_ptr(),
                Ordering::AcqRel,
                Ordering::Relaxed,
            )
            .map_err(|_| DoubleBufferInitError::AlreadyInitialized)?;
        self.buffer_len.store(buf0.len(), Ordering::Relaxed);
        self.buffer_1_ptr
            .store(buf1.as_mut_ptr(), Ordering::Relaxed);
        Ok(())
    }

    /// Retrieve buffer 0 as a mutable slice.
    ///
    /// # Safety
    ///
    /// This allows creating multiple mutable references to the same buffer. You MUST ensure
    /// that only one mutable reference exists at a time to avoid undefined behavior. In
    /// particular, for the DMA use case this type is designed for: the DMA engine must not
    /// currently be writing to this buffer, i.e. this must not be called between arming a
    /// transfer into it (via [`DmaReaderInterruptDriven::start`]/[`Self::other`]) and that transfer's
    /// completion being reported via [`DmaReaderInterruptDriven::on_interrupt`].
    #[inline]
    pub unsafe fn buffer_0(&self) -> &'static mut [u8] {
        unsafe {
            core::slice::from_raw_parts_mut(
                self.buffer_0_ptr.load(Ordering::Relaxed),
                self.buffer_len.load(Ordering::Relaxed),
            )
        }
    }

    /// Retrieve buffer 1 as a mutable slice.
    ///
    /// # Safety
    ///
    /// This allows creating multiple mutable references to the same buffer. You MUST ensure
    /// that only one mutable reference exists at a time to avoid undefined behavior. In
    /// particular, for the DMA use case this type is designed for: the DMA engine must not
    /// currently be writing to this buffer, i.e. this must not be called between arming a
    /// transfer into it (via [`DmaReaderInterruptDriven::start`]/[`Self::other`]) and that transfer's
    /// completion being reported via [`DmaReaderInterruptDriven::on_interrupt`].
    #[inline]
    pub unsafe fn buffer_1(&self) -> &'static mut [u8] {
        unsafe {
            core::slice::from_raw_parts_mut(
                self.buffer_1_ptr.load(Ordering::Relaxed),
                self.buffer_len.load(Ordering::Relaxed),
            )
        }
    }

    /// Identifies `completed` as buffer 0 or buffer 1 (by pointer) and returns the other one, so
    /// it can be armed for the next transfer. Meant to be called from the `next_buf` closure
    /// passed to [`DmaReaderInterruptDriven::on_interrupt`].
    ///
    /// # Safety
    ///
    /// Same requirement as [`Self::buffer_0`]/[`Self::buffer_1`].
    #[inline]
    pub unsafe fn other(&self, completed: &[u8]) -> &'static mut [u8] {
        if core::ptr::eq(
            completed.as_ptr(),
            self.buffer_0_ptr.load(Ordering::Relaxed),
        ) {
            unsafe { self.buffer_1() }
        } else {
            unsafe { self.buffer_0() }
        }
    }
}

/// An S2MM reader that is driven entirely by the interrupt handler.
///
/// [`Self::start`] just arms a transfer and returns immediately, and [`Self::on_interrupt`] is the
/// only place that finds out when it's done. There is no built-in buffering or double-buffering
/// scheme. You need to assemble that in your application code on top of these two primitives if
/// needed.
#[derive(Debug)]
pub struct DmaReaderInterruptDriven {
    regs: regs::direct_register::MmioS2mmRegisters<'static>,
    token: DmaInterruptRxToken,
}

impl DmaReaderInterruptDriven {
    /// The token identifying this reader's S2MM channel, fixed for the reader's whole lifetime.
    /// Retrieve it once, right after construction, to hand to your interrupt handler — no need
    /// to wait for a `start` call to return one, so it can be in place *before* the first
    /// transfer is armed and the completion interrupt can possibly fire.
    #[inline]
    pub fn token(&self) -> DmaInterruptRxToken {
        self.token
    }

    /// Starts interrupt-driven DMA reception.
    ///
    /// Programs the registers to start an S2MM transfer into `buf` but does not wait for
    /// completion. Call [`Self::on_interrupt`] with [`Self::token`] (or an equivalent one
    /// constructed via [`DmaInterruptRxToken::steal`]) from your interrupt handler to find out when
    /// it's done and get the received data back. This function will enable the interrupts.
    /// You need to ensure that an appropriate interrupt handler was configured to handle AXI
    /// DMA reception interrupts.
    pub fn start(
        &mut self,
        buf: &'static mut [u8],
    ) -> Result<DmaInterruptRxToken, InvalidBufferLengthError> {
        // Enable relevant interrupts.
        self.regs.modify_control(|val| {
            val.with_interrupt_on_complete(true)
                .with_error_interrupt_enable(true)
        });
        // Status bits latch independently of whether their matching *_IrqEn bit is set, so a
        // transfer armed earlier via the plain blocking `SimpleDmaReader` (which never
        // acknowledges them, only polls `idle()`) can leave one pending here. Clear all three
        // write-to-clear bits before arming, so enabling `interrupt_on_complete` above doesn't
        // immediately assert the interrupt line for a stale, unrelated completion — which could
        // otherwise race `start_read` below and let `on_interrupt` observe registers that don't
        // reflect this transfer yet.
        self.regs.write_status(
            regs::fields::Status::ZERO
                .with_completion_interrupt(true)
                .with_delay_timer_interrupt(true)
                .with_error_interrupt(true),
        );
        start_read(&mut self.regs, buf)?;
        Ok(self.token)
    }

    /// Services the S2MM completion interrupt for `token`. Call this from your interrupt
    /// handler.
    ///
    /// Returns `Ok(None)` if there was nothing to service (e.g. a spurious call). Otherwise
    /// acknowledges the interrupt and returns `Ok(Some(data))`, a view into the buffer passed to
    /// the triggering [`Self::start`] or the previous `on_interrupt` call, truncated to the
    /// number of bytes actually received (this can be less than that buffer's length, e.g. on a
    /// short frame terminated early via `TLAST`).
    ///
    /// If `next_buf` returns `Some`, a new transfer is armed into it before returning, so the
    /// DMA engine is never left disarmed between this frame's completion and your handler
    /// getting around to deciding what to receive into next. `next_buf` is only called once the
    /// completed data is available, and is given a shared view of it, so unlike a plain `Option`
    /// argument it can be used to pick the next buffer based on what just completed (e.g.
    /// [`DoubleBufferHelper`]'s pointer-based ping-pong: "arm whichever of my two buffers isn't
    /// this one").
    ///
    /// Returns `Err(DmaReadError::Transfer(error))` if the `error_interrupt` status bit was set
    /// instead, i.e. the transfer didn't complete cleanly. In that case only `error_interrupt` is
    /// acked, `next_buf` is not called, and the channel is left disarmed, since there's no
    /// completed data to hand off and picking a next buffer isn't this function's call to make
    /// when something already went wrong. It's up to the caller to decide how to recover.
    ///
    /// Returns `Err(DmaReadError::Rearm { data, error })` if the transfer itself completed fine
    /// but the buffer `next_buf` returned could not be armed (e.g. it was too long). The data
    /// received by the completed transfer is still handed back through `data` in that case, it's
    /// only the *next* transfer that failed to start; the channel is left disarmed until you call
    /// [`Self::start`] again.
    ///
    /// # Safety
    ///
    /// `token` must have been returned by (or constructed via [`DmaInterruptRxToken::steal`] to
    /// match) a transfer actually started on this S2MM channel.
    ///
    /// The buffer `next_buf` returns must not alias `received`: the DMA engine will start
    /// writing into it immediately, so returning the same buffer that was just completed (or
    /// any buffer overlapping it) hands the caller a `&'static mut` that hardware is
    /// concurrently writing to.
    pub unsafe fn on_interrupt(
        token: &DmaInterruptRxToken,
        next_buf: impl FnOnce(&[u8]) -> Option<&'static mut [u8]>,
    ) -> Result<Option<&'static mut [u8]>, DmaReadError> {
        let mut regs =
            unsafe { regs::direct_register::S2mmRegisters::new_mmio_at(token.base_addr) };
        let status = regs.read_status();
        let error = if status.error_interrupt() {
            regs.write_status(regs::fields::Status::ZERO.with_error_interrupt(true));
            Some(DmaTransferError {
                decode: status.dma_decode_error(),
                slave: status.dma_slave_error(),
                internal: status.dma_internal_error(),
            })
        } else {
            None
        };
        // Ack completion_interrupt whenever it's set, even if an error was also observed in the
        // same status snapshot: a stale, un-acked completion bit could otherwise keep the
        // interrupt line asserted (or get misread as a fresh completion) on a later call.
        if status.completion_interrupt() {
            regs.write_status(regs::fields::Status::ZERO.with_completion_interrupt(true));
        }
        if let Some(error) = error {
            return Err(error.into());
        }
        if !status.completion_interrupt() {
            return Ok(None);
        }
        // The dest-address register still holds what `start`/the previous `on_interrupt` call
        // programmed it to, since only the length register is updated by hardware on
        // completion; reading it back is how we recover the buffer reference that `start`'s
        // caller gave up ownership of when arming the transfer.
        let dest_addr = regs.read_dest_address_lower_word();
        let received_len = regs.read_transfer_length().length().as_usize();
        // SAFETY: `dest_addr` is the destination address of the transfer that just completed,
        // and `received_len` (<= the buffer length originally armed there) is the number of
        // bytes the DMA engine actually wrote to it. The DMA engine will not write to that
        // memory again unless a new transfer is armed into it, which only happens if `next_buf`
        // returns the very same buffer.
        let received =
            unsafe { core::slice::from_raw_parts_mut(dest_addr as *mut u8, received_len) };
        // Unlike the old `let _ = start_read(..)`, the rearm outcome is carried in the return
        // value instead of being silently dropped: a caller whose `next_buf` handed back an
        // oversized buffer needs to find out the channel is now disarmed, not just have
        // reception mysteriously stop. The received data still comes back too, through the
        // `Rearm` variant, rather than being discarded just because the *next* arm failed.
        if let Some(buf) = next_buf(&*received)
            && let Err(error) = start_read(&mut regs, buf)
        {
            return Err(DmaReadError::Rearm {
                data: received,
                error,
            });
        }
        Ok(Some(received))
    }

    /// Convenience wrapper around [`Self::on_interrupt`] for a double-buffered setup: instead of
    /// a `next_buf` closure, takes a [`DoubleBufferHelper`] and always arms whichever of its two
    /// buffers did *not* just complete, so the pair keeps alternating without you having to
    /// write that closure yourself at every call site.
    ///
    /// # Safety
    ///
    /// Same requirements as [`Self::on_interrupt`], plus: `helper` must have been initialized
    /// (its buffers assigned), and `token`'s transfer must have been armed with one of
    /// `helper`'s two buffers (via `reader.start(unsafe { helper.buffer_0() })` to start, or a
    /// previous call to this function afterwards).
    pub unsafe fn on_interrupt_double_buffered(
        token: &DmaInterruptRxToken,
        helper: &DoubleBufferHelper,
    ) -> Result<Option<&'static mut [u8]>, DmaReadError> {
        unsafe { Self::on_interrupt(token, |received| Some(helper.other(received))) }
    }
}

/// Programs the registers to start an MM2S transfer. Does not wait for completion.
///
/// Pleaes note that the source address must be aligned to the MM2S memory map data width
/// if the data realignment engine is not included.
fn start_write(
    regs: &mut regs::direct_register::MmioMm2sRegisters<'static>,
    buf: &[u8],
) -> Result<(), InvalidBufferLengthError> {
    if buf.len() > u26::MAX.as_usize() {
        return Err(InvalidBufferLengthError { len: buf.len() });
    }
    regs.modify_control(|val| val.with_run_stop(regs::fields::RunStop::Run));
    regs.write_source_address_lower_word(buf.as_ptr() as u32);
    regs.write_transfer_length(TransferLength::ZERO.with_length(u26::new(buf.len() as u32)));
    Ok(())
}

/// Programs the registers to start an S2MM transfer. Does not wait for completion.
///
/// `buf.len()` is the maximum number of bytes to receive; the stream can terminate the
/// transfer early (e.g. via `TLAST` on a short packet), so fewer bytes may end up written.
///
/// Please note that the destination address must be aligned to the S2MM memory map data width
/// if the data realignment engine is not included.
fn start_read(
    regs: &mut regs::direct_register::MmioS2mmRegisters<'static>,
    buf: &mut [u8],
) -> Result<(), InvalidBufferLengthError> {
    if buf.len() > u26::MAX.as_usize() {
        return Err(InvalidBufferLengthError { len: buf.len() });
    }
    regs.modify_control(|val| val.with_run_stop(regs::fields::RunStop::Run));
    regs.write_dest_address_lower_word(buf.as_mut_ptr() as u32);
    regs.write_transfer_length(TransferLength::ZERO.with_length(u26::new(buf.len() as u32)));
    Ok(())
}

#[cfg(test)]
mod tests {}
