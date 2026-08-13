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
#[cfg(not(feature = "portable-atomic"))]
use core::sync::atomic::AtomicBool;
use core::{cell::RefCell, convert::Infallible, marker::PhantomData};
use critical_section::Mutex;
use embassy_sync::waitqueue::AtomicWaker;
#[cfg(feature = "portable-atomic")]
use portable_atomic::AtomicBool;
use raw_buffer::RawBufSlice;

use crate::{FIFO_DEPTH, Tx};

/// 1 waker (default).
#[cfg(feature = "1-waker")]
pub const NUM_WAKERS: usize = 1;
/// 2 wakers
#[cfg(feature = "2-wakers")]
pub const NUM_WAKERS: usize = 2;
/// 4 wakers
#[cfg(feature = "4-wakers")]
pub const NUM_WAKERS: usize = 4;
/// 8 wakers
#[cfg(feature = "8-wakers")]
pub const NUM_WAKERS: usize = 8;
/// 16 wakers
#[cfg(feature = "16-wakers")]
pub const NUM_WAKERS: usize = 16;
/// 32 wakers
#[cfg(feature = "32-wakers")]
pub const NUM_WAKERS: usize = 32;
static UART_TX_WAKERS: [AtomicWaker; NUM_WAKERS] = [const { AtomicWaker::new() }; NUM_WAKERS];
static TX_CONTEXTS: [Mutex<RefCell<TxContext>>; NUM_WAKERS] =
    [const { Mutex::new(RefCell::new(TxContext::new())) }; NUM_WAKERS];
// Completion flag. Kept outside of the context structure as an atomic to avoid
// critical section.
static TX_DONE: [AtomicBool; NUM_WAKERS] = [const { AtomicBool::new(false) }; NUM_WAKERS];

/// Invalid waker index for [NUM_WAKERS].
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
/// triggered by the UARTLite using [TxAsync]. `token` should be retrieved once via
/// [TxAsync::token] right after constructing the driver.
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
    let mut uartlite_tx = unsafe { Tx::steal(token.base_addr) };
    let status = uartlite_tx.regs.read_stat_reg();
    // Interrupt are not even enabled.
    if !status.intr_enabled() {
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
    if (context.progress >= slice_len && status.tx_fifo_empty()) || slice_len == 0 {
        // Write back updated context structure.
        critical_section::with(|cs| {
            let context_ref = TX_CONTEXTS[waker_slot].borrow(cs);
            *context_ref.borrow_mut() = context;
        });
        // Transfer is done.
        TX_DONE[waker_slot].store(true, core::sync::atomic::Ordering::Relaxed);
        UART_TX_WAKERS[waker_slot].wake();
        return;
    }
    // Safety: We documented that the user provided slice must outlive the future, so we convert
    // the raw pointer back to the slice here.
    let slice = unsafe { context.slice.get() }.expect("slice is invalid");
    while context.progress < slice_len {
        if uartlite_tx.regs.read_stat_reg().tx_fifo_full() {
            break;
        }
        // Safety: TX structure is owned by the future which does not write into the the data
        // register, so we can assume we are the only one writing to the data register.
        uartlite_tx.write_fifo_unchecked(slice[context.progress]);
        context.progress += 1;
    }
    // Write back updated context structure.
    critical_section::with(|cs| {
        let context_ref = TX_CONTEXTS[waker_slot].borrow(cs);
        *context_ref.borrow_mut() = context;
    });
}

/// TX context structure.
#[derive(Debug, Copy, Clone)]
pub struct TxContext {
    progress: usize,
    slice: RawBufSlice,
}

#[allow(clippy::new_without_default)]
impl TxContext {
    /// Create a new TX context structure.
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
    tx: &'tx mut TxAsync,
    phantom: core::marker::PhantomData<&'buf ()>,
}

impl<'tx, 'buf> TxFuture<'tx, 'buf> {
    /// Create a new TX future which can be used for asynchronous TX operations.
    pub fn new(
        tx: &'tx mut TxAsync,
        waker_idx: usize,
        data: &'buf [u8],
    ) -> Result<Self, InvalidWakerIndex> {
        TX_DONE[waker_idx].store(false, core::sync::atomic::Ordering::Relaxed);
        tx.tx.reset_fifo();

        let init_fill_count = core::cmp::min(data.len(), FIFO_DEPTH);
        // We fill the FIFO with initial data.
        for data in data.iter().take(init_fill_count) {
            tx.tx.write_fifo_unchecked(*data);
        }
        critical_section::with(|cs| {
            let context_ref = TX_CONTEXTS[waker_idx].borrow(cs);
            let mut context = context_ref.borrow_mut();
            unsafe {
                context.slice.set(data);
            }
            context.progress = init_fill_count;
        });
        Ok(Self {
            waker_idx,
            tx,
            phantom: PhantomData,
        })
    }
}

impl Future for TxFuture<'_, '_> {
    type Output = usize;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        UART_TX_WAKERS[self.waker_idx].register(cx.waker());
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
        if !TX_DONE[self.waker_idx].load(core::sync::atomic::Ordering::Relaxed) {
            critical_section::with(|cs| {
                let context_ref = TX_CONTEXTS[self.waker_idx].borrow(cs);
                let mut context_mut = context_ref.borrow_mut();
                context_mut.slice.set_null();
                context_mut.progress = 0;
                // We can not disable interrupts, might be active for RX as well.
                self.tx.tx.reset_fifo();
            });
        }
    }
}

/// Asynchronous TX driver.
///
/// Relies on [on_interrupt_tx] being called with this driver's [TxToken] (see [Self::token])
/// from the UART interrupt handler: without it, futures returned by [Self::write] never make
/// progress past the initial FIFO fill and never complete.
pub struct TxAsync {
    pub(crate) tx: Tx,
    token: TxToken,
}

impl TxAsync {
    /// Create a new asynchronous TX structure.
    ///
    /// # Safety
    ///
    /// The user MUST ensure that the `Drop` method of all futures generated with this driver
    /// is called on transfer cancellation. By default, this does not require any special handling.
    /// This case was considered exotic enough to not justify an `unsafe` API.
    pub fn new(tx: Tx, waker_idx: usize) -> Result<Self, InvalidWakerIndex> {
        if waker_idx >= NUM_WAKERS {
            return Err(InvalidWakerIndex(waker_idx));
        }
        let token = TxToken {
            // Safety: only converted to a primitive address.
            base_addr: unsafe { tx.regs.ptr() } as usize,
            waker_idx,
        };
        Ok(Self { tx, token })
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
        TxFuture::new(self, self.token.waker_idx, buf).expect("waker index unexpectedly invalid")
    }

    /// Release the owned TX structure.
    pub fn release(self) -> Tx {
        self.tx
    }
}

impl embedded_io::ErrorType for TxAsync {
    type Error = Infallible;
}

impl embedded_io_async::Write for TxAsync {
    /// Write a buffer asynchronously.
    ///
    /// This implementation is not side effect free, and a started future might have already
    /// written part of the passed buffer.
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        Ok(self.write(buf).await)
    }

    /// This implementation does not do anything.
    async fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
