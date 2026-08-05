#![no_std]

use core::num::NonZero;

pub mod adc;
pub mod dac;
pub mod regs;

use adc::Adc;
use dac::Dac;
use regs::fields::R1Mode;
pub use regs::fields::{Config, FpgaInfo};

pub struct AxiAd9361 {
    base_addr_ip_core: usize,
    mmio: regs::MmioRegisters<'static>,
    adc_taken: bool,
    dac_taken: bool,
}

impl AxiAd9361 {
    /// Create a new AXI AD9361 driver.
    ///
    /// You have to provide the base address of the IP core to the constructor.
    ///
    /// # Safety
    ///
    /// - The `base_addr_ip_core` must be a valid memory-mapped register address of the
    ///   peripheral.
    /// - Dereferencing an invalid or misaligned address results in **undefined behavior**.
    /// - The caller must ensure that no other code concurrently modifies the same peripheral registers
    ///   in an unsynchronized manner to prevent data races.
    /// - This function does not enforce uniqueness of driver
    pub const fn new(base_addr_ip_core: usize) -> Self {
        let mmio = unsafe { regs::Registers::new_mmio_at(base_addr_ip_core) };
        AxiAd9361 {
            base_addr_ip_core,
            mmio,
            adc_taken: false,
            dac_taken: false,
        }
    }

    #[inline]
    pub fn read_config(&mut self) -> regs::fields::Config {
        self.mmio.read_config()
    }

    #[inline]
    pub fn read_fpga_info(&mut self) -> regs::fields::FpgaInfo {
        self.mmio.read_fpga_info()
    }

    /// Take ownership of the ADC driver, bringing it out of reset and configuring the R1 mode.
    ///
    /// Returns `None` if the ADC driver has already been taken.
    pub fn take_and_init_adc(&mut self, r1_mode: R1Mode) -> Option<Adc> {
        if self.adc_taken {
            return None;
        }
        self.adc_taken = true;
        Some(Adc::new(self.base_addr_ip_core, r1_mode))
    }

    /// Take ownership of the ADC driver without touching any of its registers.
    ///
    /// Returns `None` if the ADC driver has already been taken.
    pub const fn take_adc_no_init(&mut self) -> Option<Adc> {
        if self.adc_taken {
            return None;
        }
        self.adc_taken = true;
        Some(Adc::new_no_init(self.base_addr_ip_core))
    }

    /// Take ownership of the DAC driver, bringing it out of reset, configuring the R1 mode,
    /// the rate divider and synchronizing the channels.
    ///
    /// Returns `None` if the DAC driver has already been taken.
    pub fn take_and_init_dac(&mut self, rate_div: NonZero<u8>, r1_mode: R1Mode) -> Option<Dac> {
        if self.dac_taken {
            return None;
        }
        self.dac_taken = true;
        Some(Dac::new(self.base_addr_ip_core, rate_div, r1_mode))
    }

    /// Take ownership of the DAC driver without touching any of its registers.
    ///
    /// Returns `None` if the DAC driver has already been taken.
    pub const fn take_dac_no_init(&mut self) -> Option<Dac> {
        if self.dac_taken {
            return None;
        }
        self.dac_taken = true;
        Some(Dac::new_no_init(self.base_addr_ip_core))
    }
}

#[cfg(test)]
mod tests {}
