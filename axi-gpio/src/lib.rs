//! # AMD AXI GPIO driver
//!
//! This is a native Rust driver for the
//! [AMD AXI GPIO IP core](https://www.amd.com/en/products/adaptive-socs-and-fpgas/intellectual-property/axi_gpio.html).
//!
//! # Features
//!
//! If the asynchronous support for is used, the number of statically provided wakers
//! can be configured using the following features:
//!
//! - `1-waker` which is the default
//! - `2-wakers`
//! - `4-wakers`
//! - `8-wakers`
//! - `16-wakers`
//! - `32-wakers`
//!
//! The number of required wakers is the total number of ports with interrupts enabled.
//! For example, when using one AXI GPIO block with interrupt support on both ports and a second
//! one with interrupt support on only one port, you would require at least 3 wakers, so you
//! could select the `4-wakers` feature.
//!
//! Additionally:
//!
//! - `defmt` implements `defmt::Format` for this crate's register and error types.
#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(missing_docs)]

use core::marker::PhantomData;

use arbitrary_int::{traits::Integer as _, u5};

pub mod asynch;
pub mod regs;

/// AXI GPIO peripheral driver.
pub struct AxiGpio {
    regs: regs::MmioRegisters<'static>,
    ch1_events_taken: bool,
    ch2_events_taken: bool,
}

impl AxiGpio {
    /// Create a new AXI GPIO peripheral driver for an IP core instance configured with only
    /// one channel block, together with pin ownership tokens for that channel.
    ///
    /// # Safety
    ///
    /// - The `base_addr` must be a valid memory-mapped register address of an AXI GPIO peripheral.
    /// - Dereferencing an invalid or misaligned address results in **undefined behavior**.
    /// - The caller must ensure that no other code concurrently modifies the same peripheral registers
    ///   in an unsynchronized manner to prevent data races.
    /// - This function does not enforce uniqueness of driver instances. Creating multiple instances
    ///   with the same `base_addr` can lead to unintended behavior if not externally synchronized.
    /// - The driver performs **volatile** reads and writes to the provided address.
    pub const unsafe fn new_single_channel(base_addr: u32) -> (Self, ChannelPins<Channel1Marker>) {
        (unsafe { Self::from_base_addr(base_addr) }, unsafe {
            ChannelPins::steal()
        })
    }

    /// Create a new AXI GPIO peripheral driver for an IP core instance configured with both
    /// channels, together with pin ownership tokens for both of them.
    ///
    /// See [Self::new_single_channel] for what using this on a single-channel instance does.
    ///
    /// # Safety
    ///
    /// - The `base_addr` must be a valid memory-mapped register address of an AXI GPIO peripheral.
    /// - Dereferencing an invalid or misaligned address results in **undefined behavior**.
    /// - The caller must ensure that no other code concurrently modifies the same peripheral registers
    ///   in an unsynchronized manner to prevent data races.
    /// - This function does not enforce uniqueness of driver instances. Creating multiple instances
    ///   with the same `base_addr` can lead to unintended behavior if not externally synchronized.
    /// - The driver performs **volatile** reads and writes to the provided address.
    pub const unsafe fn new_dual_channel(
        base_addr: u32,
    ) -> (
        Self,
        ChannelPins<Channel1Marker>,
        ChannelPins<Channel2Marker>,
    ) {
        (
            unsafe { Self::from_base_addr(base_addr) },
            unsafe { ChannelPins::steal() },
            unsafe { ChannelPins::steal() },
        )
    }

    const unsafe fn from_base_addr(base_addr: u32) -> Self {
        let regs = unsafe { regs::Registers::new_mmio_at(base_addr as usize) };
        Self {
            regs,
            ch1_events_taken: false,
            ch2_events_taken: false,
        }
    }

    /// Direct register access.
    #[inline(always)]
    pub const fn regs(&mut self) -> &mut regs::MmioRegisters<'static> {
        &mut self.regs
    }

    /// Configure a pin as an [Input].
    pub fn input_pin<Ch: AxiGpioChannel, const N: usize>(&self, pin: Pin<Ch, N>) -> Input {
        Input::new(&self.regs, pin)
    }

    /// Configure a pin as an [Output], driving `init_level` immediately.
    pub fn output_pin<Ch: AxiGpioChannel, const N: usize>(
        &self,
        pin: Pin<Ch, N>,
        init_level: embedded_hal::digital::PinState,
    ) -> Output {
        Output::new(&self.regs, pin, init_level)
    }

    /// Enable the peripheral's global interrupt output.
    ///
    /// Has no effect unless at least one channel also has its interrupt enabled via
    /// [Self::enable_channel_interrupt].
    pub fn enable_global_interrupt(&mut self) {
        self.regs.write_global_interrupt_enable(
            regs::fields::GlobalInterruptEnable::builder()
                .with_enable(true)
                .build(),
        );
    }

    /// Disable the peripheral's global interrupt output.
    pub fn disable_global_interrupt(&mut self) {
        self.regs.write_global_interrupt_enable(
            regs::fields::GlobalInterruptEnable::builder()
                .with_enable(false)
                .build(),
        );
    }

    /// Enable the interrupt for one channel.
    ///
    /// The AXI GPIO core only has one interrupt bit per channel: it fires when any pin of that
    /// channel changes, see [asynch] for what that means for async support. Don't call this for
    /// a channel that has an [asynch::AsyncChannel]: it manages this same bit itself, and the
    /// two would fight over it.
    pub fn enable_channel_interrupt(&mut self, channel: ChannelId) {
        self.regs.modify_interrupt_enable(|val| match channel {
            ChannelId::Ch1 => val.with_channel1(true),
            ChannelId::Ch2 => val.with_channel2(true),
        });
    }

    /// Disable the interrupt for one channel.
    pub fn disable_channel_interrupt(&mut self, channel: ChannelId) {
        self.regs.modify_interrupt_enable(|val| match channel {
            ChannelId::Ch1 => val.with_channel1(false),
            ChannelId::Ch2 => val.with_channel2(false),
        });
    }

    /// Create an asynchronous, interrupt-driven event source for one channel.
    ///
    /// Returns `Ok(None)` if this channel's events were already taken from this [AxiGpio]
    /// instance - each channel's events can only be taken once per instance.
    ///
    /// See [asynch::AsyncChannel] for details and the interrupt handler wiring this requires to
    /// make progress.
    pub fn event_channel(
        &mut self,
        channel: ChannelId,
        waker_idx: usize,
    ) -> Result<Option<asynch::AsyncChannel>, asynch::ClaimWakerError> {
        let taken = match channel {
            ChannelId::Ch1 => &mut self.ch1_events_taken,
            ChannelId::Ch2 => &mut self.ch2_events_taken,
        };
        if *taken {
            return Ok(None);
        }
        // The waker claim below can still fail, so only commit to `taken` once it succeeds.
        let events = asynch::AsyncChannel::new(&self.regs, channel, waker_idx)?;
        *taken = true;
        Ok(Some(events))
    }
}

/// Individual channel identifier on an AXI GPIO block.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ChannelId {
    /// Channel 1.
    Ch1,
    /// Channel 2.
    Ch2,
}

/// Type-level marker for channel 1.
pub struct Channel1Marker {}
/// Type-level marker for channel 2.
pub struct Channel2Marker {}

/// Marker trait implemented by channel markers.
pub trait AxiGpioChannel {
    /// Actual channel ID.
    const ID: ChannelId;
}

impl AxiGpioChannel for Channel1Marker {
    const ID: ChannelId = ChannelId::Ch1;
}

impl AxiGpioChannel for Channel2Marker {
    const ID: ChannelId = ChannelId::Ch2;
}

/// Marker type representing ownership of pin `N` of a GPIO channel on a [AxiGpioChannel].
#[derive(Debug)]
pub struct Pin<Ch: AxiGpioChannel, const N: usize>(PhantomData<Ch>);

impl<Ch: AxiGpioChannel, const N: usize> Pin<Ch, N> {
    /// Create new pin resource structure.
    const fn new() -> Self {
        Self(PhantomData)
    }

    /// Steal a pin resource structure.
    ///
    /// # Safety
    ///
    /// This can duplicate a resource tracking structure, which in turn can lead to data races
    /// when using other APIs relying on ownership guarantees. Furthermore, `N` must be smaller
    /// or equal to 31.
    pub const unsafe fn steal() -> Self {
        Self(PhantomData)
    }
}

/// Ownership tokens for the 32 pins of one AXI GPIO channel.
#[derive(Debug)]
pub struct ChannelPins<Ch: AxiGpioChannel> {
    /// GPIO pin 0.
    pub p0: Pin<Ch, 0>,
    /// GPIO pin 1.
    pub p1: Pin<Ch, 1>,
    /// GPIO pin 2.
    pub p2: Pin<Ch, 2>,
    /// GPIO pin 3.
    pub p3: Pin<Ch, 3>,
    /// GPIO pin 4.
    pub p4: Pin<Ch, 4>,
    /// GPIO pin 5.
    pub p5: Pin<Ch, 5>,
    /// GPIO pin 6.
    pub p6: Pin<Ch, 6>,
    /// GPIO pin 7.
    pub p7: Pin<Ch, 7>,
    /// GPIO pin 8.
    pub p8: Pin<Ch, 8>,
    /// GPIO pin 9.
    pub p9: Pin<Ch, 9>,
    /// GPIO pin 10.
    pub p10: Pin<Ch, 10>,
    /// GPIO pin 11.
    pub p11: Pin<Ch, 11>,
    /// GPIO pin 12.
    pub p12: Pin<Ch, 12>,
    /// GPIO pin 13.
    pub p13: Pin<Ch, 13>,
    /// GPIO pin 14.
    pub p14: Pin<Ch, 14>,
    /// GPIO pin 15.
    pub p15: Pin<Ch, 15>,
    /// GPIO pin 16.
    pub p16: Pin<Ch, 16>,
    /// GPIO pin 17.
    pub p17: Pin<Ch, 17>,
    /// GPIO pin 18.
    pub p18: Pin<Ch, 18>,
    /// GPIO pin 19.
    pub p19: Pin<Ch, 19>,
    /// GPIO pin 20.
    pub p20: Pin<Ch, 20>,
    /// GPIO pin 21.
    pub p21: Pin<Ch, 21>,
    /// GPIO pin 22.
    pub p22: Pin<Ch, 22>,
    /// GPIO pin 23.
    pub p23: Pin<Ch, 23>,
    /// GPIO pin 24.
    pub p24: Pin<Ch, 24>,
    /// GPIO pin 25.
    pub p25: Pin<Ch, 25>,
    /// GPIO pin 26.
    pub p26: Pin<Ch, 26>,
    /// GPIO pin 27.
    pub p27: Pin<Ch, 27>,
    /// GPIO pin 28.
    pub p28: Pin<Ch, 28>,
    /// GPIO pin 29.
    pub p29: Pin<Ch, 29>,
    /// GPIO pin 30.
    pub p30: Pin<Ch, 30>,
    /// GPIO pin 31.
    pub p31: Pin<Ch, 31>,
}

impl<Ch: AxiGpioChannel> ChannelPins<Ch> {
    /// Steal a new set of pin ownership tokens for channel `Ch`.
    ///
    /// # Safety
    ///
    /// Only one set of tokens must exist per channel at a time. The caller must ensure that no
    /// other [ChannelPins] for the same channel is alive, since nothing here prevents handing
    /// out tokens for pins that are already configured and in use elsewhere.
    pub const unsafe fn steal() -> Self {
        Self {
            p0: Pin::new(),
            p1: Pin::new(),
            p2: Pin::new(),
            p3: Pin::new(),
            p4: Pin::new(),
            p5: Pin::new(),
            p6: Pin::new(),
            p7: Pin::new(),
            p8: Pin::new(),
            p9: Pin::new(),
            p10: Pin::new(),
            p11: Pin::new(),
            p12: Pin::new(),
            p13: Pin::new(),
            p14: Pin::new(),
            p15: Pin::new(),
            p16: Pin::new(),
            p17: Pin::new(),
            p18: Pin::new(),
            p19: Pin::new(),
            p20: Pin::new(),
            p21: Pin::new(),
            p22: Pin::new(),
            p23: Pin::new(),
            p24: Pin::new(),
            p25: Pin::new(),
            p26: Pin::new(),
            p27: Pin::new(),
            p28: Pin::new(),
            p29: Pin::new(),
            p30: Pin::new(),
            p31: Pin::new(),
        }
    }
}

/// Low-level, unchecked access to a single GPIO pin.
///
/// Data and tri-state are shared 32-bit registers across all 32 pins of a channel, and the AXI
/// GPIO core has no atomic set/clear/toggle registers, so a pin operation is a plain
/// read-modify-write of that shared register. Concurrently accessing different pins of the same
/// channel from different execution contexts (e.g. thread and interrupt handler) is a data race
/// and must be externally synchronized.
pub struct LowLevelGpio {
    regs: regs::MmioRegisters<'static>,
    channel: ChannelId,
    offset: u5,
    /// Forces `!Send` regardless of what `regs::MmioRegisters` happens to contain, since a pin
    /// handed to another execution context is exactly the unsynchronized-access hazard described
    /// above.
    _not_send: core::marker::PhantomData<*const ()>,
}

impl LowLevelGpio {
    /// Create a new low-level pin handle from a [Pin] ownership token.
    pub const fn new<Ch: AxiGpioChannel, const N: usize>(
        regs: &regs::MmioRegisters<'static>,
        _pin: Pin<Ch, N>,
    ) -> Self {
        // Safety: `pin` is a `Pin<N>` ownership token, so this (channel, offset) is genuinely
        // owned here.
        unsafe { Self::steal(regs, Ch::ID, u5::new(N as u8)) }
    }

    /// Create a new low-level pin handle without requiring a [Pin] ownership token.
    ///
    /// Prefer [Self::new] where possible.
    ///
    /// # Safety
    ///
    /// The caller must ensure no other handle (`LowLevelGpio`, [Input], [Output], or another
    /// [Self::steal]) already exists for the same `(channel, offset)` pin, for the same reason
    /// documented on [ChannelPins::steal]. `offset` must also be less than 32, or a later read
    /// or write through this handle panics inside the bitfield bounds check.
    pub const unsafe fn steal(
        regs: &regs::MmioRegisters<'static>,
        channel: ChannelId,
        offset: u5,
    ) -> Self {
        Self {
            regs: unsafe { regs.clone() },
            channel,
            offset,
            _not_send: core::marker::PhantomData,
        }
    }

    #[inline(always)]
    fn read_data(&self) -> regs::fields::Pins {
        match self.channel {
            ChannelId::Ch1 => self.regs.read_ch1_data(),
            ChannelId::Ch2 => self.regs.read_ch2_data(),
        }
    }

    #[inline(always)]
    fn modify_data<F: FnOnce(regs::fields::Pins) -> regs::fields::Pins>(&mut self, f: F) {
        match self.channel {
            ChannelId::Ch1 => self.regs.modify_ch1_data(f),
            ChannelId::Ch2 => self.regs.modify_ch2_data(f),
        }
    }

    #[inline(always)]
    fn modify_tri_state<F: FnOnce(regs::fields::Pins) -> regs::fields::Pins>(&mut self, f: F) {
        match self.channel {
            ChannelId::Ch1 => self.regs.modify_ch1_tri_state(f),
            ChannelId::Ch2 => self.regs.modify_ch2_tri_state(f),
        }
    }

    /// Configure the pin as an input.
    ///
    /// The tri-state register bit is set to 1, which tri-states (releases) the output driver.
    pub fn configure_as_input_pin(&mut self) {
        let offset = self.offset;
        self.modify_tri_state(|val| val.with_pins(offset.as_usize(), true));
    }

    /// Configure the pin as an output.
    ///
    /// The tri-state register bit is cleared to 0, which enables the output driver.
    pub fn configure_as_output_pin(&mut self) {
        let offset = self.offset;
        self.modify_tri_state(|val| val.with_pins(offset.as_usize(), false));
    }

    /// Read the sampled input level.
    #[inline(always)]
    pub fn is_high(&self) -> bool {
        self.read_data().pins(self.offset.as_usize())
    }

    /// Read the sampled input level.
    #[inline(always)]
    pub fn is_low(&self) -> bool {
        !self.is_high()
    }

    /// Drive the pin high.
    #[inline(always)]
    pub fn set_high(&mut self) {
        let offset = self.offset;
        self.modify_data(|val| val.with_pins(offset.as_usize(), true));
    }

    /// Drive the pin low.
    #[inline(always)]
    pub fn set_low(&mut self) {
        let offset = self.offset;
        self.modify_data(|val| val.with_pins(offset.as_usize(), false));
    }

    /// Toggle the driven pin level.
    #[inline(always)]
    pub fn toggle(&mut self) {
        let offset = self.offset;
        self.modify_data(|val| {
            let cur = val.pins(offset.as_usize());
            val.with_pins(offset.as_usize(), !cur)
        });
    }

    /// Read back the last driven output level.
    ///
    /// AXI GPIO has a single data register for both directions, so this reads the same register
    /// as [Self::is_high].
    #[inline(always)]
    pub fn is_set_high(&self) -> bool {
        self.is_high()
    }

    /// Read back the last driven output level.
    #[inline(always)]
    pub fn is_set_low(&self) -> bool {
        !self.is_set_high()
    }
}

/// GPIO output pin driver.
pub struct Output(LowLevelGpio);

impl Output {
    /// Configure a [Pin] as an output, driving `init_level` immediately.
    ///
    /// It is recommended to use [AxiGpio::output_pin] to retrieve an instance of an output pin.
    pub fn new<Ch: AxiGpioChannel, const N: usize>(
        regs: &regs::MmioRegisters<'static>,
        pin: Pin<Ch, N>,
        init_level: embedded_hal::digital::PinState,
    ) -> Self {
        let mut pin = LowLevelGpio::new(regs, pin);
        match init_level {
            embedded_hal::digital::PinState::Low => pin.set_low(),
            embedded_hal::digital::PinState::High => pin.set_high(),
        }
        pin.configure_as_output_pin();
        Self(pin)
    }

    /// Drive the pin high.
    #[inline]
    pub fn set_high(&mut self) {
        self.0.set_high();
    }

    /// Drive the pin low.
    #[inline]
    pub fn set_low(&mut self) {
        self.0.set_low();
    }

    /// Toggle the driven pin level.
    #[inline]
    pub fn toggle(&mut self) {
        self.0.toggle();
    }
}

impl embedded_hal::digital::ErrorType for Output {
    type Error = core::convert::Infallible;
}

impl embedded_hal::digital::OutputPin for Output {
    #[inline]
    fn set_high(&mut self) -> Result<(), Self::Error> {
        self.set_high();
        Ok(())
    }

    #[inline]
    fn set_low(&mut self) -> Result<(), Self::Error> {
        self.set_low();
        Ok(())
    }
}

impl embedded_hal::digital::StatefulOutputPin for Output {
    #[inline]
    fn is_set_high(&mut self) -> Result<bool, Self::Error> {
        Ok(self.0.is_set_high())
    }

    #[inline]
    fn is_set_low(&mut self) -> Result<bool, Self::Error> {
        Ok(self.0.is_set_low())
    }
}

/// GPIO input pin driver.
pub struct Input(LowLevelGpio);

impl Input {
    /// Configure a [Pin] as an input.
    ///
    /// It is recommended to use [AxiGpio::input_pin] to retrieve an instance of an input pin.
    pub fn new<Ch: AxiGpioChannel, const N: usize>(
        regs: &regs::MmioRegisters<'static>,
        pin: Pin<Ch, N>,
    ) -> Self {
        let mut pin = LowLevelGpio::new(regs, pin);
        pin.configure_as_input_pin();
        Self(pin)
    }

    /// Read the sampled input level.
    #[inline]
    pub fn is_high(&self) -> bool {
        self.0.is_high()
    }

    /// Read the sampled input level.
    #[inline]
    pub fn is_low(&self) -> bool {
        self.0.is_low()
    }
}

impl embedded_hal::digital::ErrorType for Input {
    type Error = core::convert::Infallible;
}

impl embedded_hal::digital::InputPin for Input {
    #[inline]
    fn is_high(&mut self) -> Result<bool, Self::Error> {
        Ok(self.0.is_high())
    }

    #[inline]
    fn is_low(&mut self) -> Result<bool, Self::Error> {
        Ok(self.0.is_low())
    }
}

#[cfg(test)]
mod tests {}
