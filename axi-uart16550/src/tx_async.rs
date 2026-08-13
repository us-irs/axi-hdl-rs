//! # Asynchronous TX support.
//!
//! This module provides support for asynchronous non-blocking TX transfers.
//!
//! It provides a static number of async wakers to allow a configurable amount of pollable
//! [TxFuture]s. Each UARTLite [Tx] instance which performs asynchronous TX operations needs
//! to be to explicitely assigned a waker when creating an awaitable [TxAsync] structure
//! as well as when calling the [on_interrupt_tx] handler.
//!
//! The maximum number of available wakers is configured via the waker feature flags:
//!
//! - `1-waker`
//! - `2-wakers`
//! - `4-wakers`
//! - `8-wakers`
//! - `16-wakers`
//! - `32-wakers`
#[cfg(not(feature = "portable-atomic"))]
use core::sync::atomic::AtomicBool;
use core::{cell::RefCell, convert::Infallible};
#[cfg(feature = "portable-atomic")]
use portable_atomic::AtomicBool;

use critical_section::Mutex;
use embassy_sync::waitqueue::AtomicWaker;
use embedded_hal_async::delay::DelayNs;
use raw_buffer::RawBufSlice;

use crate::{
    FIFO_DEPTH, Tx,
    registers::{self, InterruptEnable},
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
static TX_CONTEXTS: [Mutex<RefCell<TxContext>>; NUM_WAKERS] =
    [const { Mutex::new(RefCell::new(TxContext::new())) }; NUM_WAKERS];

// Completion flag. Kept outside of the context structure as an atomic to avoid
// critical section.
static TX_DONE: [AtomicBool; NUM_WAKERS] = [const { AtomicBool::new(false) }; NUM_WAKERS];

/// Invalid waker index error.
#[derive(Debug, thiserror::Error)]
#[error("invalid waker slot index: {0}")]
pub struct InvalidWakerIndex(pub usize);

/// This is a generic interrupt handler to handle asynchronous UART TX operations for a given
/// UART peripheral.
///
/// The user has to call this once in the interrupt handler responsible if the interrupt was
/// triggered by the UARTLite. The relevant [Tx] handle of the UARTLite and the waker slot used
/// for it must be passed as well. [Tx::steal] can be used to create the required handle.
pub fn on_interrupt_tx(tx: &mut Tx, waker_slot: usize) {
    if waker_slot >= NUM_WAKERS {
        return;
    }
    let status = tx.regs.read_lsr();
    let ier = InterruptEnable::new_with_raw_value(tx.regs.read_ier_or_dlm());
    // Interrupt are not even enabled.
    if !ier.thr_empty() {
        return;
    }
    let mut context = critical_section::with(|cs| {
        let context_ref = TX_CONTEXTS[waker_slot].borrow(cs);
        *context_ref.borrow()
    });
    // No transfer active.
    if context.slice.is_null() {
        return;
    }
    let slice_len = context.slice.len().unwrap();
    // We have to use the THRE instead of the TEMT status flag here, because the interrupt
    // is configured to trigger on the THRE flag and the UART might still be busy shifting the
    // last byte out.
    if (context.progress >= slice_len && status.thr_empty()) || slice_len == 0 {
        // Write back updated context structure.
        critical_section::with(|cs| {
            let context_ref = TX_CONTEXTS[waker_slot].borrow(cs);
            *context_ref.borrow_mut() = context;
        });
        // Transfer is done.
        TX_DONE[waker_slot].store(true, core::sync::atomic::Ordering::Relaxed);
        tx.disable_interrupt();
        WAKERS[waker_slot].wake();
        return;
    }
    // Safety: We documented that the user provided slice must outlive the future, so we convert
    // the raw pointer back to the slice here.
    let slice = unsafe { context.slice.get() }.expect("slice is invalid");
    while context.progress < slice_len {
        match tx.write_fifo(slice[context.progress]) {
            Ok(_) => context.progress += 1,
            Err(nb::Error::WouldBlock) => break,
        }
    }
    // Write back updated context structure.
    critical_section::with(|cs| {
        let context_ref = TX_CONTEXTS[waker_slot].borrow(cs);
        *context_ref.borrow_mut() = context;
    });
}

#[derive(Debug, Copy, Clone)]
struct TxContext {
    progress: usize,
    slice: RawBufSlice,
}

#[allow(clippy::new_without_default)]
impl TxContext {
    pub const fn new() -> Self {
        Self {
            progress: 0,
            slice: RawBufSlice::new_nulled(),
        }
    }
}

/// TX future structure.
pub struct TxFuture<'tx, 'buf> {
    waker_idx: usize,
    reg_block: registers::MmioRegisters<'static>,
    phantom: core::marker::PhantomData<(&'tx (), &'buf ())>,
}

impl<'tx, 'buf> TxFuture<'tx, 'buf> {
    /// Create a new TX future which can be used for asynchronous TX operations.
    pub fn new(tx: &mut Tx, waker_idx: usize, data: &'buf [u8]) -> Result<Self, InvalidWakerIndex> {
        TX_DONE[waker_idx].store(false, core::sync::atomic::Ordering::Relaxed);
        tx.disable_interrupt();
        tx.reset_fifo();

        let init_fill_count = core::cmp::min(data.len(), FIFO_DEPTH);
        critical_section::with(|cs| {
            let context_ref = TX_CONTEXTS[waker_idx].borrow(cs);
            let mut context = context_ref.borrow_mut();
            unsafe {
                context.slice.set(data);
            }
            context.progress = init_fill_count;
        });
        // We fill the FIFO with initial data.
        for data in data.iter().take(init_fill_count) {
            tx.write_fifo_unchecked(*data);
        }
        tx.enable_interrupt();
        Ok(Self {
            waker_idx,
            reg_block: unsafe { tx.regs.clone() },
            phantom: core::marker::PhantomData,
        })
    }
}

impl Future for TxFuture<'_, '_> {
    type Output = usize;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        WAKERS[self.waker_idx].register(cx.waker());
        if TX_DONE[self.waker_idx].swap(false, core::sync::atomic::Ordering::Relaxed) {
            let progress = critical_section::with(|cs| {
                let mut ctx = TX_CONTEXTS[self.waker_idx].borrow(cs).borrow_mut();
                ctx.slice.set_null();
                ctx.progress
            });
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
        // for this waker slot can never dereference it.
        if !TX_DONE[self.waker_idx].load(core::sync::atomic::Ordering::Relaxed) {
            critical_section::with(|cs| {
                let context_ref = TX_CONTEXTS[self.waker_idx].borrow(cs);
                let mut context_mut = context_ref.borrow_mut();
                context_mut.slice.set_null();
                context_mut.progress = 0;
            });
        }
    }
}

/// Asynchronous TX driver.
pub struct TxAsync<D: DelayNs> {
    tx: Tx,
    waker_idx: usize,
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
        Ok(Self {
            tx,
            waker_idx,
            delay,
        })
    }

    /// Write a buffer asynchronously.
    ///
    /// This implementation is not side effect free, and a started future might have already
    /// written part of the passed buffer.
    pub fn write<'buf>(&mut self, buf: &'buf [u8]) -> TxFuture<'_, 'buf> {
        TxFuture::new(&mut self.tx, self.waker_idx, buf).unwrap()
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
