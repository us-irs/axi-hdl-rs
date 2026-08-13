//! # Asynchronous TX support.
//!
//! This module provides support for asynchronous non-blocking TX transfers.
//!
//! It provides a static number of async wakers to allow a configurable amount of pollable
//! [TxFuture]s. Each UARTLite [Tx] instance which performs asynchronous TX operations needs
//! to be to explicitely assigned a waker when creating an awaitable [TxAsync] structure.
//! Retrieve the resulting [TxToken] via [TxAsync::token] right after construction and pass it
//! to [on_interrupt_tx] from your interrupt handler.
//!
//! The maximum number of available wakers is configured via the waker feature flags:
//!
//! - `1-waker`
//! - `2-wakers`
//! - `4-wakers`
//! - `8-wakers`
//! - `16-wakers`
//! - `32-wakers`
use core::convert::Infallible;
#[cfg(not(feature = "portable-atomic"))]
use core::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};
#[cfg(feature = "portable-atomic")]
use portable_atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};

use embassy_sync::waitqueue::AtomicWaker;
use embedded_hal_async::delay::DelayNs;

use crate::{
    FIFO_DEPTH, Tx,
    regs::{self, fields::InterruptEnable},
};

/// 1 waker (default).
#[cfg(feature = "1-waker")]
pub const NUM_WAKERS: usize = 1;
/// 2 wakers.
#[cfg(feature = "2-wakers")]
pub const NUM_WAKERS: usize = 2;
/// 4 wakers.
#[cfg(feature = "4-wakers")]
pub const NUM_WAKERS: usize = 4;
/// 8 wakers.
#[cfg(feature = "8-wakers")]
pub const NUM_WAKERS: usize = 8;
/// 16 wakers.
#[cfg(feature = "16-wakers")]
pub const NUM_WAKERS: usize = 16;
/// 32 wakers.
#[cfg(feature = "32-wakers")]
pub const NUM_WAKERS: usize = 32;

static WAKERS: [AtomicWaker; NUM_WAKERS] = [const { AtomicWaker::new() }; NUM_WAKERS];
static TX_CONTEXTS: [TxContext; NUM_WAKERS] = [const { TxContext::new() }; NUM_WAKERS];

// Completion flag. Kept outside of the context structure as an atomic to avoid
// critical section.
static TX_DONE: [AtomicBool; NUM_WAKERS] = [const { AtomicBool::new(false) }; NUM_WAKERS];

/// Invalid waker index error.
#[derive(Debug, thiserror::Error)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[error("invalid waker slot index: {0}")]
pub struct InvalidWakerIndex(pub usize);

/// Identifies a [TxAsync] driver's UART instance and waker slot, e.g. for use in an interrupt
/// handler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct TxToken {
    base_addr: usize,
    waker_idx: usize,
}

impl TxToken {
    /// The UART register block's base address.
    #[inline]
    pub fn base_addr(&self) -> usize {
        self.base_addr
    }

    /// The waker slot this token's TX driver was constructed with.
    #[inline]
    pub fn waker_idx(&self) -> usize {
        self.waker_idx
    }

    /// Constructs a token from a raw base address and waker index, e.g. for use in an interrupt
    /// handler that only has these two values available from static configuration, rather than
    /// a token retrieved via [TxAsync::token].
    ///
    /// # Safety
    ///
    /// The caller must ensure `base_addr` is the real base address of the UART register block
    /// whose TX interrupt is being serviced, and that `waker_idx` matches the slot originally
    /// passed to the corresponding [TxAsync::new] call.
    #[inline]
    pub const unsafe fn steal(base_addr: usize, waker_idx: usize) -> Self {
        Self {
            base_addr,
            waker_idx,
        }
    }
}

/// This is a generic interrupt handler to handle asynchronous UART TX operations for a given
/// UART peripheral.
///
/// The user has to call this once in the interrupt handler responsible if the interrupt was
/// triggered by the UART. `token` should be retrieved once via [TxAsync::token] right after
/// constructing the driver.
///
/// # Safety
///
/// `token` must have been returned by [TxAsync::token] (or constructed via [TxToken::steal] to
/// match) for a TX driver actually performing the transfer being serviced.
pub unsafe fn on_interrupt_tx(token: &TxToken) {
    if token.waker_idx >= NUM_WAKERS {
        return;
    }
    let waker_slot = token.waker_idx;
    let mut tx = unsafe { Tx::steal(token.base_addr) };
    let status = tx.regs.read_lsr();
    let ier = InterruptEnable::new_with_raw_value(tx.regs.read_ier_or_dlm());
    // Interrupt are not even enabled.
    if !ier.thr_empty() {
        return;
    }
    let context = &TX_CONTEXTS[waker_slot];
    // `Acquire` pairs with the `Release` store in `TxFuture::new`/`poll`/`Drop`: seeing a
    // non-null pointer here guarantees `transfer_len`/`progress` below are the values published
    // together with it, not stale ones from a previous transfer.
    let raw_data_ptr = context.raw_data.load(Ordering::Acquire) as *const u8;
    // No transfer active.
    if raw_data_ptr.is_null() {
        return;
    }
    let slice_len = context.transfer_len.load(Ordering::Relaxed);
    let mut progress = context.progress.load(Ordering::Relaxed);
    // Safety: We documented that the user provided slice must outlive the future, so we convert
    // the raw pointer back to the slice here.
    let slice = unsafe { core::slice::from_raw_parts(raw_data_ptr, slice_len) };
    // We have to use the THRE instead of the TEMT status flag here, because the interrupt
    // is configured to trigger on the THRE flag and the UART might still be busy shifting the
    // last byte out.
    if (progress >= slice_len && status.thr_empty()) || slice_len == 0 {
        // Transfer is done. `Release` publishes the final `progress` value to whichever context
        // observes `TX_DONE` via the `Acquire` swap in `poll`.
        TX_DONE[waker_slot].store(true, Ordering::Release);
        tx.disable_interrupt();
        WAKERS[waker_slot].wake();
        return;
    }
    while progress < slice_len {
        match tx.write_fifo(slice[progress]) {
            Ok(_) => progress += 1,
            Err(nb::Error::WouldBlock) => break,
        }
    }
    context.progress.store(progress, Ordering::Relaxed);
}

/// TX context structure. Plain atomics rather than a `critical_section::Mutex<RefCell<_>>` so it
/// can live in a `static` array directly. `raw_data` doubles as the "transfer active" flag: it
/// is always published last (`Release`) after `transfer_len`/`progress`, and read first
/// (`Acquire`) before them, so a reader that observes it non-null is guaranteed to see the
/// matching, not stale, `transfer_len`/`progress`.
struct TxContext {
    progress: AtomicUsize,
    raw_data: AtomicPtr<u8>,
    transfer_len: AtomicUsize,
}

impl TxContext {
    const fn new() -> Self {
        Self {
            progress: AtomicUsize::new(0),
            raw_data: AtomicPtr::new(core::ptr::null_mut()),
            transfer_len: AtomicUsize::new(0),
        }
    }
}

/// TX future structure.
pub struct TxFuture<'tx, 'buf> {
    waker_idx: usize,
    reg_block: regs::MmioRegisters<'static>,
    // Set once `poll` observes completion. `TX_DONE` itself is not enough to tell completion
    // and cancellation apart in `Drop`, because `poll` already swaps it back to `false` as
    // part of observing it.
    completed: bool,
    phantom: core::marker::PhantomData<(&'tx (), &'buf ())>,
}

impl<'tx, 'buf> TxFuture<'tx, 'buf> {
    /// Create a new TX future which can be used for asynchronous TX operations.
    pub fn new(tx: &mut Tx, waker_idx: usize, data: &'buf [u8]) -> Result<Self, InvalidWakerIndex> {
        TX_DONE[waker_idx].store(false, Ordering::Relaxed);
        tx.disable_interrupt();
        tx.reset_fifo();

        let init_fill_count = core::cmp::min(data.len(), FIFO_DEPTH);
        let context_ref = &TX_CONTEXTS[waker_idx];
        // Publish the guarded fields before opening the gate (`raw_data`) with `Release`, so a
        // reader that observes `raw_data` non-null via the `Acquire` load in `on_interrupt_tx`
        // is guaranteed to see these too, rather than stale values from a previous transfer.
        context_ref
            .transfer_len
            .store(data.len(), Ordering::Relaxed);
        context_ref
            .progress
            .store(init_fill_count, Ordering::Relaxed);
        context_ref
            .raw_data
            .store(data.as_ptr() as *mut u8, Ordering::Release);
        // We fill the FIFO with initial data.
        for data in data.iter().take(init_fill_count) {
            tx.write_fifo_unchecked(*data);
        }
        tx.enable_interrupt();
        Ok(Self {
            waker_idx,
            reg_block: unsafe { tx.regs.clone() },
            completed: false,
            phantom: core::marker::PhantomData,
        })
    }
}

impl Future for TxFuture<'_, '_> {
    type Output = usize;

    fn poll(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        WAKERS[self.waker_idx].register(cx.waker());
        if TX_DONE[self.waker_idx].swap(false, Ordering::Acquire) {
            let context = &TX_CONTEXTS[self.waker_idx];
            context
                .raw_data
                .store(core::ptr::null_mut(), Ordering::Release);
            let progress = context.progress.load(Ordering::Relaxed);
            self.completed = true;
            return core::task::Poll::Ready(progress);
        }
        core::task::Poll::Pending
    }
}

impl Drop for TxFuture<'_, '_> {
    fn drop(&mut self) {
        let mut tx = Tx::new(unsafe { self.reg_block.clone() });
        tx.disable_interrupt();
        // On cancellation, clear the stale buffer pointer so a spurious or future interrupt
        // for this waker slot can never dereference it. `self.completed` (set inside `poll`'s
        // `Ready` arm) is what actually distinguishes cancellation from normal completion here,
        // since `TX_DONE` itself is already swapped back to `false` by the time a completed
        // future is dropped.
        if !self.completed {
            let context_ref = &TX_CONTEXTS[self.waker_idx];
            context_ref.progress.store(0, Ordering::Relaxed);
            context_ref
                .raw_data
                .store(core::ptr::null_mut(), Ordering::Release);
        }
    }
}

/// Asynchronous TX driver.
///
/// Relies on [on_interrupt_tx] being called with this driver's [TxToken] (see [Self::token])
/// from the UART interrupt handler: without it, futures returned by [Self::write] never make
/// progress past the initial FIFO fill and never complete.
pub struct TxAsync<D: DelayNs> {
    tx: Tx,
    token: TxToken,
    delay: D,
}

impl<D: DelayNs> TxAsync<D> {
    /// Create a new asynchronous TX structure.
    ///
    /// The delay function is a [DelayNs] provider which is used to allow flushing the
    /// device properly. This is because even when a write finished, the UART might still
    /// be busy shifting the last byte out.
    ///
    /// # Safety
    ///
    /// The user MUST ensure that the `Drop` method of all futures generated with this driver
    /// is called on transfer cancellation. By default, this does not require any special handling.
    /// This case was considered exotic enough to not justify an `unsafe` API.
    pub fn new(tx: Tx, waker_idx: usize, delay: D) -> Result<Self, InvalidWakerIndex> {
        if waker_idx >= NUM_WAKERS {
            return Err(InvalidWakerIndex(waker_idx));
        }
        let token = TxToken {
            // Safety: only converted to a primitive address.
            base_addr: unsafe { tx.regs.ptr() } as usize,
            waker_idx,
        };
        Ok(Self { tx, token, delay })
    }

    /// The token identifying this driver's UART instance and waker slot, fixed for its whole
    /// lifetime. Retrieve it once, right after construction, to hand to [on_interrupt_tx] in
    /// your interrupt handler.
    ///
    /// Since the token needs to reach a separate interrupt context, a crate like `once_cell` can
    /// be used to share it safely.
    #[inline]
    pub fn token(&self) -> TxToken {
        self.token
    }

    /// Write a buffer asynchronously.
    ///
    /// This implementation is not side effect free, and a started future might have already
    /// written part of the passed buffer.
    pub fn write<'buf>(&mut self, buf: &'buf [u8]) -> TxFuture<'_, 'buf> {
        TxFuture::new(&mut self.tx, self.token.waker_idx, buf).unwrap()
    }

    /// Flush this output stream, ensuring that all intermediately buffered contents reach their destination.
    pub async fn flush(&mut self) {
        while !self.tx.tx_empty() {
            self.delay.delay_us(10).await;
        }
    }

    /// Release the underlying TX handle.
    pub fn release(self) -> Tx {
        self.tx
    }
}

impl<D: DelayNs> embedded_io::ErrorType for TxAsync<D> {
    type Error = Infallible;
}

impl<D: DelayNs> embedded_io_async::Write for TxAsync<D> {
    /// Write a buffer asynchronously.
    ///
    /// This implementation is not side effect free, and a started future might have already
    /// written part of the passed buffer.
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        Ok(self.write(buf).await)
    }

    /// Flush this output stream, ensuring that all intermediately buffered contents reach their destination.
    async fn flush(&mut self) -> Result<(), Self::Error> {
        self.flush().await;
        Ok(())
    }
}
