use arbitrary_int::{traits::Integer as _, u4};

use crate::regs::{self, fields::R1Mode};

pub struct Adc {
    mmio: regs::adc::MmioRegisters<'static>,
}

impl Adc {
    /// Create a new ADC driver.
    ///
    /// You have to provide the base address of the IP core to the constructor. This needs to be
    /// the base address of the IP core without the ADC block offset.
    /// This function also brings the ADC core out of reset.
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
    pub fn new(base_addr_ip_core: usize, r1_mode: R1Mode) -> Self {
        let mut adc = Self::new_no_init(base_addr_ip_core);
        adc.mmio
            .modify_adc_control1(|val| val.with_r1_mode(r1_mode));
        adc.enable();
        let num_channels = match r1_mode {
            R1Mode::OneChannel => 2,
            R1Mode::TwoChannels => 4,
        };
        for ch in 0..num_channels {
            adc.enable_channel(u4::new(ch));
        }
        adc
    }

    pub const fn new_no_init(base_addr_ip_core: usize) -> Self {
        Adc {
            mmio: regs::Registers::new_adc_block(base_addr_ip_core),
        }
    }

    pub fn enable(&mut self) {
        self.mmio.write_reset(crate::regs::fields::Reset::ZERO);
        self.mmio.write_reset(crate::regs::fields::Reset::RELEASED);
    }

    /// Pulses the core reset without dropping the MMCM out of lock, to force the digital
    /// interface to relatch after e.g. an interface-delay register write. Unlike [`Self::enable`],
    /// this does not fully re-assert reset first.
    pub fn reset_pulse(&mut self) {
        self.mmio.write_reset(
            crate::regs::fields::Reset::ZERO
                .with_clock_disable(false)
                .with_mmcm_reset_n(true),
        );
        self.mmio.write_reset(crate::regs::fields::Reset::RELEASED);
    }

    pub fn channel_mut(&mut self, channel: u4) -> regs::adc::MmioChannel<'_> {
        self.mmio
            .adc_channels(channel.as_usize())
            .expect("ADC channel retrieval failed unexpectedly")
    }

    /// Enables a single channel's data path (`format_signext`/`format_enable`/`enable`),
    /// matching the per-channel `CHAN_CNTRL` write in `axi_adc_init()`. Without this, the
    /// channel's datapath — and anything tapping it, like the PN monitor — never activates.
    pub fn enable_channel(&mut self, channel: u4) {
        let mut ch = self.channel_mut(channel);
        ch.write_data_path_control(
            regs::adc::fields::ChannelDataPathControl::ZERO
                .with_format_signext(true)
                .with_format_enable(true)
                .with_enable(true)
                // Mirrors the C driver's `ad9361_post_setup()`, which sets `AXI_ADC_IQCOR_ENB`
                // unconditionally in the same per-channel enable write, not just transiently
                // during `digital_tune_tx`. With the identity coefficients written below this
                // should be a mathematical no-op, but it's not necessarily a no-op at the
                // circuit level (the correction stage may add pipeline latency even at
                // identity), so match C exactly instead of assuming it doesn't matter.
                .with_iqcor_enable(true),
        );
        // Mirrors the C driver's `ad9361_post_setup()`: initialize the IQ-correction
        // coefficients (CNTRL_2, `up_adc_iqcor_coeff_1`/`_2` in up_adc_channel.v, packed as
        // coeff_1 in bits [31:16] and coeff_2 in bits [15:0], Q1.14 fixed point) to the identity
        // matrix -- coeff_1=1.0 for even (I) channels, coeff_2=1.0 for odd (Q) channels. This
        // register otherwise stays at its power-on-reset value (0), which would silently zero
        // the data path the moment anything (e.g. digital_tune_tx) turns `iqcor_enable` on.
        ch.write_control2(if channel.value() % 2 == 0 {
            0x4000_0000
        } else {
            0x0000_4000
        });
    }

    #[inline]
    pub fn read_status(&mut self) -> regs::adc::fields::Status {
        self.mmio.read_adc_status()
    }

    #[inline]
    pub fn regs(&mut self) -> &regs::adc::MmioRegisters<'static> {
        &self.mmio
    }

    #[inline]
    pub fn regs_mut(&mut self) -> &mut regs::adc::MmioRegisters<'static> {
        &mut self.mmio
    }
}
