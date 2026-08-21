//! # Asynchronous, interrupt-driven channel events.
//!
//! The AXI GPIO core only exposes one interrupt bit per channel: it fires when *any* pin of
//! that channel changes, and gives no indication of which pin, or which edge. There is no
//! per-pin interrupt enable or mask register in this IP core, unlike some other GPIO
//! controllers. So the only thing this module can offer is "channel had an event", not "pin N
//! went high" - a caller wanting per-pin resolution has to diff the returned snapshot against
//! the previous one itself.
//!
//! This module provides a static number of async wakers to allow a configurable amount of
//! pollable [ChannelEventFuture]s, one per channel with interrupts enabled. Retrieve the
//! resulting [ChannelToken] via [AsyncChannel::token] right after construction and pass it to
//! [on_interrupt] from your interrupt handler.
//!
//! The maximum number of available wakers is configured via the waker feature flags documented
//! at the crate root.
#[cfg(not(feature = "portable-atomic"))]
use core::sync::atomic::{AtomicBool, Ordering};
#[cfg(feature = "portable-atomic")]
use portable_atomic::{AtomicBool, Ordering};

use embassy_sync::waitqueue::AtomicWaker;

use crate::{ChannelId, regs};

/// 1 waker (default).
///
/// Select exactly one of the `*-waker(s)` features. More than one is a duplicate-definition
/// compile error (every one of them defines this same constant); none at all leaves it
/// undefined.
#[cfg(feature = "1-waker")]
pub const NUM_WAKERS: usize = 1;
/// 2 wakers, see [NUM_WAKERS]'s `1-waker` docs.
#[cfg(feature = "2-wakers")]
pub const NUM_WAKERS: usize = 2;
/// 4 wakers, see [NUM_WAKERS]'s `1-waker` docs.
#[cfg(feature = "4-wakers")]
pub const NUM_WAKERS: usize = 4;
/// 8 wakers, see [NUM_WAKERS]'s `1-waker` docs.
#[cfg(feature = "8-wakers")]
pub const NUM_WAKERS: usize = 8;
/// 16 wakers, see [NUM_WAKERS]'s `1-waker` docs.
#[cfg(feature = "16-wakers")]
pub const NUM_WAKERS: usize = 16;
/// 32 wakers, see [NUM_WAKERS]'s `1-waker` docs.
#[cfg(feature = "32-wakers")]
pub const NUM_WAKERS: usize = 32;

static CHANNEL_WAKERS: [AtomicWaker; NUM_WAKERS] = [const { AtomicWaker::new() }; NUM_WAKERS];
// Event flag. Kept outside of any context structure as an atomic to avoid a critical section.
static CHANNEL_DONE: [AtomicBool; NUM_WAKERS] = [const { AtomicBool::new(false) }; NUM_WAKERS];
/// Global ownership table for waker slots, shared by every [AxiGpio](crate::AxiGpio) instance,
/// since `CHANNEL_WAKERS`/`CHANNEL_DONE` are global too. Claimed atomically via [claim_waker].
static CHANNEL_WAKER_TAKEN: [AtomicBool; NUM_WAKERS] =
    [const { AtomicBool::new(false) }; NUM_WAKERS];

/// Error returned by [AsyncChannel::new] when claiming a waker slot fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ClaimWakerError {
    /// `waker_idx` is out of range for [NUM_WAKERS].
    #[error("invalid waker slot index: {0}")]
    InvalidWakerIndex(usize),
    /// `waker_idx` was already claimed by another live [AsyncChannel].
    #[error("waker slot index {0} is already in use by another AsyncChannel")]
    WakerIndexInUse(usize),
}

/// Atomically claims `waker_idx` in the global [CHANNEL_WAKER_TAKEN] table. There is no matching
/// release: like the rest of this crate's `steal`/`take`-style ownership, a claim lasts for the
/// program's lifetime.
fn claim_waker(waker_idx: usize) -> Result<(), ClaimWakerError> {
    if waker_idx >= NUM_WAKERS {
        return Err(ClaimWakerError::InvalidWakerIndex(waker_idx));
    }
    CHANNEL_WAKER_TAKEN[waker_idx]
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| ClaimWakerError::WakerIndexInUse(waker_idx))?;
    Ok(())
}

/// Identifies an [AsyncChannel]'s peripheral instance, channel, and waker slot, e.g. for use in
/// an interrupt handler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ChannelToken {
    base_addr: usize,
    channel: ChannelId,
    waker_idx: usize,
}

impl ChannelToken {
    /// The GPIO register block's base address.
    #[inline]
    pub const fn base_addr(&self) -> usize {
        self.base_addr
    }

    /// The channel this token's [AsyncChannel] was constructed for.
    #[inline]
    pub const fn channel(&self) -> ChannelId {
        self.channel
    }

    /// The waker slot this token's [AsyncChannel] was constructed with.
    #[inline]
    pub const fn waker_idx(&self) -> usize {
        self.waker_idx
    }

    /// Constructs a token from a raw base address, channel, and waker index, e.g. for use in an
    /// interrupt handler that only has these values available from static configuration, rather
    /// than a token retrieved via [AsyncChannel::token].
    ///
    /// # Safety
    ///
    /// The caller must ensure `base_addr` is the real base address of the GPIO register block
    /// whose interrupt is being serviced, and that `channel`/`waker_idx` match the values
    /// originally passed to the corresponding [AsyncChannel::new] call.
    #[inline]
    pub const unsafe fn steal(base_addr: usize, channel: ChannelId, waker_idx: usize) -> Self {
        Self {
            base_addr,
            channel,
            waker_idx,
        }
    }
}

/// Generic interrupt handler for a channel's asynchronous events.
///
/// The user has to call this once in the interrupt handler responsible if the interrupt was
/// triggered by the GPIO channel tracked by `token`. `token` should be retrieved once via
/// [AsyncChannel::token] right after constructing the driver. Does nothing if `token`'s channel
/// has no interrupt pending, or if channel interrupts are not enabled.
///
/// # Safety
///
/// `token` must have been returned by [AsyncChannel::token] (or constructed via
/// [ChannelToken::steal] to match) for the channel actually being serviced.
pub unsafe fn on_interrupt(token: &ChannelToken) {
    if token.waker_idx >= NUM_WAKERS {
        return;
    }
    let mut regs = unsafe { regs::Registers::new_mmio_at(token.base_addr) };
    let enabled = regs.read_interrupt_enable();
    let status = regs.read_interrupt_status();
    let (enabled, pending) = match token.channel {
        ChannelId::Ch1 => (enabled.channel1(), status.channel1()),
        ChannelId::Ch2 => (enabled.channel2(), status.channel2()),
    };
    if !enabled || !pending {
        return;
    }
    // Acknowledge only this channel's bit. `interrupt_status` toggles on write, so a 0 bit is a
    // no-op and leaves the other channel's pending status untouched.
    let ack = match token.channel {
        ChannelId::Ch1 => regs::fields::InterruptBits::builder()
            .with_channel1(true)
            .with_channel2(false)
            .build(),
        ChannelId::Ch2 => regs::fields::InterruptBits::builder()
            .with_channel1(false)
            .with_channel2(true)
            .build(),
    };
    regs.write_interrupt_status(ack);
    CHANNEL_DONE[token.waker_idx].store(true, Ordering::Release);
    CHANNEL_WAKERS[token.waker_idx].wake();
}

/// Asynchronous, interrupt-driven channel event source.
///
/// Relies on [on_interrupt] being called with this driver's [ChannelToken] (see
/// [Self::token]) from the GPIO interrupt handler: without it, futures returned by [Self::wait]
/// never complete.
///
/// The caller is also responsible for enabling the global interrupt, e.g. via
/// [crate::AxiGpio::enable_global_interrupt]: the per-channel interrupt is enabled and disabled
/// automatically, for the lifetime of each [Self::wait] call only, so it stays off (no dead
/// interrupts firing for nobody) whenever nothing is actually waiting. This assumes only one
/// [AsyncChannel] exists per channel at a time - since the per-channel enable bit is shared
/// hardware state, a second one for the same channel (or manually toggling it via
/// [crate::AxiGpio::enable_channel_interrupt]/`disable_channel_interrupt` while an [AsyncChannel]
/// for it exists) would fight over it.
pub struct AsyncChannel {
    regs: regs::MmioRegisters<'static>,
    token: ChannelToken,
    /// See [crate::LowLevelGpio]'s equivalent field for why this exists.
    _not_send: core::marker::PhantomData<*const ()>,
}

impl AsyncChannel {
    /// Create a new asynchronous channel event source.
    pub fn new(
        regs: &regs::MmioRegisters<'static>,
        channel: ChannelId,
        waker_idx: usize,
    ) -> Result<Self, ClaimWakerError> {
        claim_waker(waker_idx)?;
        let regs = unsafe { regs.clone() };
        let token = ChannelToken {
            // Safety: only converted to a primitive address.
            base_addr: unsafe { regs.ptr() } as usize,
            channel,
            waker_idx,
        };
        Ok(Self {
            regs,
            token,
            _not_send: core::marker::PhantomData,
        })
    }

    /// The token identifying this driver's GPIO instance, channel, and waker slot, fixed for its
    /// whole lifetime. Retrieve it once, right after construction, to hand to
    /// [on_interrupt] in your interrupt handler.
    ///
    /// Since the token needs to reach a separate interrupt context, a crate like `once_cell` can
    /// be used to share it safely.
    #[inline]
    pub const fn token(&self) -> ChannelToken {
        self.token
    }

    /// Wait for the next interrupt event on this channel.
    ///
    /// Resolves to a snapshot of the channel's data register taken right after the event fired.
    /// Since the hardware only signals "something on this channel changed", a caller that needs
    /// to know which pin changed has to keep its own previous snapshot and diff against it.
    ///
    /// Enables this channel's interrupt for the lifetime of the returned future, and disables it
    /// again once the future is dropped, see [AsyncChannel] for why.
    pub fn wait(&mut self) -> ChannelEventFuture<'_> {
        ChannelEventFuture::new(self)
    }

    #[inline]
    fn set_channel_interrupt_enabled(&mut self, enabled: bool) {
        self.regs
            .modify_interrupt_enable(|val| match self.token.channel {
                ChannelId::Ch1 => val.with_channel1(enabled),
                ChannelId::Ch2 => val.with_channel2(enabled),
            });
    }
}

/// Future returned by [AsyncChannel::wait].
pub struct ChannelEventFuture<'a> {
    channel: &'a mut AsyncChannel,
}

impl<'a> ChannelEventFuture<'a> {
    fn new(channel: &'a mut AsyncChannel) -> Self {
        // Ordering matters: clear the flag before enabling, so a status bit already latched from
        // before this call (interrupt disabled, but the hardware condition still recorded, see
        // `on_interrupt`) re-fires immediately once enabled below rather than racing this store.
        CHANNEL_DONE[channel.token.waker_idx].store(false, Ordering::Relaxed);
        channel.set_channel_interrupt_enabled(true);
        Self { channel }
    }
}

impl core::future::Future for ChannelEventFuture<'_> {
    type Output = regs::fields::Pins;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        let waker_idx = self.channel.token.waker_idx;
        CHANNEL_WAKERS[waker_idx].register(cx.waker());
        if CHANNEL_DONE[waker_idx].swap(false, Ordering::Acquire) {
            let data = match self.channel.token.channel {
                ChannelId::Ch1 => self.channel.regs.read_ch1_data(),
                ChannelId::Ch2 => self.channel.regs.read_ch2_data(),
            };
            return core::task::Poll::Ready(data);
        }
        core::task::Poll::Pending
    }
}

impl Drop for ChannelEventFuture<'_> {
    fn drop(&mut self) {
        // Runs on both normal completion and cancellation - `poll` returning `Ready` drops the
        // future right after, same as dropping it while still `Pending` does.
        self.channel.set_channel_interrupt_enabled(false);
    }
}
