use core::num::NonZero;

use arbitrary_int::{traits::Integer as _, u4};

use crate::regs::{self, fields::R1Mode};

pub struct Dac {
    mmio: regs::dac::MmioRegisters<'static>,
}

impl Dac {
    /// Create a new DAC driver.
    ///
    /// You have to provide the base address of the IP core to the constructor. This needs to be
    /// the base address of the IP core without the DAC block offset.
    /// This function also enables the driver and synchronizes the channels within the DAC.
    ///
    /// # Safety
    ///
    /// - The `base_addr_ip_core` must be a valid memory-mapped register address of the
    ///   peripheral.
    /// - Dereferencing an invalid or misaligned address results in **undefined behavior**.
    /// - The caller must ensure that no other code concurrently modifies the same peripheral registers
    ///   in an unsynchronized manner to prevent data races.
    /// - This function does not enforce uniqueness of driver instances. Creating multiple instances
    ///   with the same `base_addr` can lead to unintended behavior if not externally synchronized.
    /// - The driver performs **volatile** reads and writes to the provided address.
    pub fn new(base_addr_ip_core: usize, rate_div: NonZero<u8>, r1_mode: R1Mode) -> Self {
        let mut dac = Self::new_no_init(base_addr_ip_core);
        dac.mmio.modify_control2(|val| val.with_r1_mode(r1_mode));
        dac.enable();
        dac.set_rate_div(rate_div);
        dac.synchronize();
        dac
    }

    /// Create a new DAC driver without touching any registers.
    ///
    /// This only constructs the register access handle at the given base address. The caller
    /// is responsible for bringing the DAC out of reset, configuring the R1 mode, setting the
    /// rate divider and synchronizing the channels, e.g. by calling [`Self::enable`],
    /// [`Self::set_rate_div`] and [`Self::synchronize`] as needed.
    ///
    /// # Safety
    ///
    /// See the safety section of [`Self::new`].
    pub const fn new_no_init(base_addr_ip_core: usize) -> Self {
        Dac {
            mmio: regs::Registers::new_dac_block(base_addr_ip_core),
        }
    }

    pub fn enable(&mut self) {
        self.mmio.write_reset(crate::regs::fields::Reset::ZERO);
        self.release_reset();
    }

    /// Releases the DAC core reset in a single write, matching the unconditional
    /// `RSTN | MMCM_RSTN` write the C driver issues at TX digital-tune entry
    /// (`ad9361_dig_tune_tx()`). Unlike [`Adc::reset_pulse`](crate::adc::Adc::reset_pulse), this
    /// does not assert reset first — it's a defensive "make sure the core is out of reset" write,
    /// and [`Self::enable`] already does this (plus more) at construction time.
    pub fn release_reset(&mut self) {
        self.mmio.write_reset(crate::regs::fields::Reset::RELEASED);
    }

    /// Also synchronizes the DAC channels.
    pub fn set_data_source(&mut self, channel: u4, source: regs::dac::regs::DataSource) {
        self.mmio
            .dac_channels(channel.as_usize())
            .expect("DAC channel retrieval failed unexpectedly")
            .write_data_source(
                regs::dac::regs::ChannelDataSource::builder()
                    .with_data_source(source)
                    .build(),
            );
        self.synchronize();
    }

    /// Also synchronizes the DAC channels.
    pub fn set_data_source_all_channels_up_to(
        &mut self,
        num_channels: u4,
        source: regs::dac::regs::DataSource,
    ) {
        for i in 0..num_channels.as_usize() {
            self.mmio
                .dac_channels(i)
                .expect("DAC channel retrieval failed unexpectedly")
                .write_data_source(
                    regs::dac::regs::ChannelDataSource::builder()
                        .with_data_source(source)
                        .build(),
                );
        }
        self.synchronize();
    }

    /// Sets the DAC's rate divider: `dac_valid` pulses (and thus new samples) are consumed at
    /// `dac_clk / rate_div`. `rate_div == 1` means no division (full rate, every `dac_clk`
    /// cycle).
    ///
    /// ADI's own docs describe this loosely as "samples are generated at 1/RATE of the
    /// interface clock", but per the `dac_rate_cnt` countdown in `axi_ad9361_tx.v` the
    /// `RATECNTRL.RATE` register actually holds `rate_div - 1`: the core reloads the counter to
    /// `RATE` and counts down through `RATE + 1` states before emitting a pulse, so
    /// `effective_rate = dac_clk / (RATE + 1)`. This matches the register's `RATE == 0` reset
    /// default meaning "full rate" rather than a division-by-zero special case, and matches the
    /// no-OS reference driver, which writes `init->rate` unmodified into this register and sets
    /// it to `1` (i.e. divide-by-2) for the 1R1T / [`R1Mode::OneChannel`] case
    /// (`tx_dac_init.rate = 1` in `no-OS/projects/ad9361/src/main.c`).
    pub fn set_rate_div(&mut self, rate_div: NonZero<u8>) {
        self.mmio
            .write_rate_control(regs::dac::regs::RateControl::new_with_raw_value(
                (rate_div.get() - 1) as u32,
            ));
    }

    pub fn channel_mut(&mut self, channel: u4) -> regs::dac::MmioChannel<'_> {
        self.mmio
            .dac_channels(channel.as_usize())
            .expect("DAC channel retrieval failed unexpectedly")
    }

    pub fn synchronize(&mut self) {
        self.mmio
            .write_control1(regs::dac::regs::Control1::ZERO.with_sync(true));
    }

    #[inline]
    pub fn read_interface_status(&mut self) -> u32 {
        self.mmio.read_interface_status()
    }

    #[inline]
    pub fn regs(&mut self) -> &mut regs::dac::MmioRegisters<'static> {
        &mut self.mmio
    }
}
