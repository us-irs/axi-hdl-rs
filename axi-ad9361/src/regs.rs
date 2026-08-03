pub mod fields {
    #[bitbybit::bitfield(
        u32,
        default = 0,
        debug,
        defmt_bitfields(feature = "defmt"),
        forbid_overlaps
    )]
    pub struct Reset {
        /// Set to 0x0 to enable the clock. Clock enabled by default.
        #[bit(2, rw)]
        clock_disable: bool,
        /// Software must write 1 to bring up core.
        #[bit(1, rw)]
        mmcm_reset_n: bool,
        /// Software must write 1 to bring up core.
        #[bit(0, rw)]
        reset_n: bool,
    }

    #[bitbybit::bitenum(u1, exhaustive = true)]
    #[derive(Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum InterfaceType {
        Sdr = 0,
        Ddr = 1,
    }

    #[bitbybit::bitenum(u1, exhaustive = true)]
    #[derive(Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum SymbolModeBits {
        _8 = 1,
        _16 = 0,
    }

    #[bitbybit::bitenum(u1, exhaustive = true)]
    #[derive(Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum R1Mode {
        OneChannel = 1,
        TwoChannels = 0,
    }

    #[bitbybit::bitfield(
        u32,
        default = 0,
        debug,
        defmt_bitfields(feature = "defmt"),
        forbid_overlaps
    )]
    pub struct Config {
        #[bit(11, rw)]
        rd_raw_data: bool,
        #[bit(10, rw)]
        external_sync: bool,
        #[bit(9, rw)]
        scale_correction_only: bool,
        #[bit(8, rw)]
        pps_receiver: bool,
        #[bit(7, rw)]
        cmos_or_lvds: bool,
        #[bit(6, rw)]
        dds_disable: bool,
        #[bit(5, rw)]
        delay_control_disable: bool,
        #[bit(4, rw)]
        mode_1r1t: bool,

        #[bit(3, rw)]
        userports_disabled: bool,
        #[bit(2, rw)]
        dataformat_disabled: bool,
        #[bit(1, r)]
        dc_filter_disabled: bool,
        #[bit(0, r)]
        iq_correction_disabled: bool,
    }

    #[bitbybit::bitfield(
        u32,
        default = 0,
        debug,
        defmt_bitfields(feature = "defmt"),
        forbid_overlaps
    )]
    pub struct FpgaInfo {
        #[bits(24..=31, rw)]
        technology: u8,
        #[bits(16..=23, rw)]
        family: u8,
        #[bits(8..=15, rw)]
        speed: u8,
        #[bits(0..=7, rw)]
        dev_package: u8,
    }
}

pub mod adc {
    pub use crate::regs::fields::Reset;

    pub mod fields {
        use arbitrary_int::u5;

        pub use crate::regs::fields::{InterfaceType, R1Mode, SymbolModeBits};

        #[bitbybit::bitenum(u1, exhaustive = true)]
        #[derive(Debug, PartialEq, Eq)]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        pub enum DdrEdgeSelect {
            Rising = 0,
            Falling = 1,
        }

        #[bitbybit::bitenum(u1, exhaustive = true)]
        #[derive(Debug, PartialEq, Eq)]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        pub enum PinMode {
            ClockMultiplexed = 1,
            PinMultiplexed = 0,
        }

        #[bitbybit::bitfield(
            u32,
            default = 0,
            debug,
            defmt_bitfields(feature = "defmt"),
            forbid_overlaps
        )]
        pub struct Control1 {
            #[bit(16, rw)]
            interface_type: InterfaceType,
            /// Select symbol data format mode.
            #[bit(15, rw)]
            symb_op: bool,
            #[bit(14, rw)]
            symb_8_16b: SymbolModeBits,
            #[bits(8..=12, rw)]
            num_of_lanes: u5,
            #[bit(3, rw)]
            sync: bool,
            #[bit(2, rw)]
            r1_mode: R1Mode,
            #[bit(1, rw)]
            ddr_edgesel: DdrEdgeSelect,
            #[bit(0, rw)]
            pin_mode: PinMode,
        }
    }

    #[derive(derive_mmio::Mmio)]
    #[repr(C)]
    pub struct Channel {
        control0: u32,
        status: u32,
        raw_data: u32,
        control1: u32,
        control2: u32,
        control3: u32,
        user_control1: u32,
        user_control2: u32,
    }

    static_assertions::const_assert_eq!(core::mem::size_of::<Channel>(), 0x20);

    #[derive(derive_mmio::Mmio)]
    #[repr(C)]
    pub struct Registers {
        resets: Reset,
        adc_control1: fields::Control1,
        adc_control2: u32,
        adc_control3: u32,
        _gap2: u32,
        adc_clock_freq: u32,
        adc_clock_ratio: u32,
        #[mmio(PureRead)]
        adc_status: u32,
        adc_delay_control: u32,
        #[mmio(PureRead)]
        adc_delay_status: u32,
        #[mmio(PureRead)]
        adc_sync_status: u32,
        _gap3: u32,
        adc_drp_control: u32,
        #[mmio(PureRead)]
        adc_drp_status: u32,
        adc_drp_wdata: u32,
        #[mmio(PureRead)]
        adc_drp_rdata: u32,
        adc_config_write: u32,
        #[mmio(PureRead)]
        adc_config_read: u32,
        ui_status: u32,
        adc_config_control: u32,
        _gap4: [u32; 0x04],
        user_control_1: u32,
        adc_start_code: u32,
        _gap5: [u32; 0x04],
        adc_gpio_in: u32,
        adc_gpio_out: u32,
        pps_counter: u32,
        pps_status: u32,

        _gap6: [u32; 0xCE],

        #[mmio(Inner)]
        adc_channels: [Channel; 16],
    }
}

pub mod dac {
    use crate::regs::dac::regs::{Control1, Control2, RateControl};
    pub use crate::regs::fields::Reset;

    pub mod regs {
        pub use arbitrary_int::u5;

        pub use crate::regs::fields::{InterfaceType, R1Mode, SymbolModeBits};

        #[bitbybit::bitenum(u1, exhaustive = true)]
        #[derive(Debug, PartialEq, Eq)]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        pub enum ParityType {
            Even = 0,
            Odd = 1,
        }

        #[bitbybit::bitenum(u1, exhaustive = true)]
        #[derive(Debug, PartialEq, Eq)]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        pub enum ParityMode {
            Frame = 0,
            Parity = 1,
        }

        #[bitbybit::bitfield(
            u32,
            default = 0,
            debug,
            defmt_bitfields(feature = "defmt"),
            forbid_overlaps
        )]
        pub struct Control1 {
            #[bit(3, rw)]
            manual_sync_request: bool,
            #[bit(2, rw)]
            disarm_ext_sync: bool,
            #[bit(1, rw)]
            arm_ext_sync: bool,
            #[bit(0, rw)]
            sync: bool,
        }

        #[bitbybit::bitfield(
            u32,
            default = 0,
            debug,
            defmt_bitfields(feature = "defmt"),
            forbid_overlaps
        )]
        pub struct Control2 {
            #[bit(16, rw)]
            interface_type: InterfaceType,
            /// Select symbol data format mode.
            #[bit(15, rw)]
            symb_op: bool,
            #[bit(14, rw)]
            symb_8_16b: SymbolModeBits,
            #[bits(8..=12, rw)]
            num_of_lanes: u5,
            #[bit(7, rw)]
            parity_type: ParityType,
            #[bit(6, rw)]
            parity_mode: ParityType,
            #[bit(5, rw)]
            r1_mode: R1Mode,
            #[bit(4, rw)]
            data_format: bool,
        }

        #[bitbybit::bitfield(
            u32,
            default = 0,
            debug,
            defmt_bitfields(feature = "defmt"),
            forbid_overlaps
        )]
        pub struct RateControl {
            #[bits(0..=7, rw)]
            rate: u8,
        }

        #[bitbybit::bitenum(u4, exhaustive = false)]
        #[derive(Debug, PartialEq, Eq)]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        pub enum DataSource {
            /// Internal tone (DDS).
            InternalTone = 0x00,
            /// Pattern (SED).
            Pattern = 0x01,
            // Input Data (DMA (?)).
            InputData = 0x02,
            Zero = 0x03,
            InvertedPn7 = 0x04,
            InvertedPn15 = 0x05,
            Pn7 = 0x06,
            Pn15 = 0x07,
            LoopbackDataAdc = 0x08,
            /// Device specific.
            PnX = 0x09,
            /// Device specific.
            NibbleRamp = 0x0A,
            /// Device specific.
            BitRam16Bit = 0x0B,
        }

        #[bitbybit::bitfield(
            u32,
            default = 0,
            debug,
            defmt_bitfields(feature = "defmt"),
            forbid_overlaps
        )]
        pub struct ChannelControl7 {
            #[bits(0..=3, rw)]
            data_source: Option<DataSource>,
        }
    }

    #[derive(derive_mmio::Mmio)]
    #[repr(C)]
    pub struct Channel {
        control1: u32,
        control2: u32,
        control3: u32,
        control4: u32,
        control5: u32,
        control6: u32,
        control7: regs::ChannelControl7,
        control8: u32,
        user_control3: u32,
        user_control4: u32,
        user_control5: u32,
        control9: u32,
        control10: u32,
        _gap0: [u32; 0x3],
    }

    static_assertions::const_assert_eq!(core::mem::size_of::<Channel>(), 0x40);

    #[derive(derive_mmio::Mmio)]
    #[repr(C)]
    pub struct Registers {
        // DAC registers.
        _gap8: [u32; 0x10],
        reset: Reset,
        control1: Control1,
        control2: Control2,
        rate_control: RateControl,
        frame: u32,
        status1: u32,
        dac_status2: u32,
        /// Bit 0: interface status. Set if there are no errors; if unset, there are errors and
        /// software may try resetting the core.
        #[mmio(PureRead)]
        dac_status3: u32,
        dac_clksel: u32,
        _gap9: u32,
        dac_sync_status: u32,
        _gap10: u32,
        dac_drp_control: u32,
        dac_drp_status: u32,
        dac_drp_wdata: u32,
        dac_drp_rdata: u32,
        dac_custom_read: u32,
        dac_custom_write: u32,
        dac_ui_status: u32,
        dac_custom_control: u32,
        _gap11: [u32; 4],
        dac_user_control_1: u32,
        _gap12: [u32; 5],
        dac_gpio_in: u32,
        dac_gpio_out: u32,

        _gap13: [u32; 0xD0],

        #[mmio(Inner)]
        dac_channels: [Channel; 16],
    }
}

#[derive(derive_mmio::Mmio)]
#[repr(C)]
pub struct Registers {
    version: u32,
    id: u32,
    scratch: u32,
    #[mmio(PureRead)]
    config: fields::Config,
    pps_irq_mask: u32,
    _gap0: [u32; 0x02],
    fpga_info: fields::FpgaInfo,
    _gap1: [u32; 0x08],

    // ADC registers.
    #[mmio(Inner)]
    adc: adc::Registers,

    _gap7: [u32; 0x280],

    #[mmio(Inner)]
    dac: dac::Registers,

    _gap14: [u32; 0x200],

    // TDD registers.
    _gap15: [u32; 0x10],
    tdd_control0: u32,
    tdd_control1: u32,
    tdd_control2: u32,
    tdd_frame_length: u32,
    tdd_sync_terminal_type: u32,
    _gap16: [u32; 0x03],
    tdd_status: u32,
    _gap17: [u32; 0x07],

    tdd_vco_rx_on_1: u32,
    tdd_vco_rx_off_1: u32,
    tdd_vco_tx_on_1: u32,
    tdd_vco_tx_off_1: u32,

    tdd_rx_on_1: u32,
    tdd_rx_off_1: u32,
    tdd_tx_on_1: u32,
    tdd_tx_off_1: u32,

    tdd_rx_dp_on_1: u32,
    tdd_rx_dp_off_1: u32,
    tdd_tx_dp_on_1: u32,
    tdd_tx_dp_off_1: u32,

    _gap18: [u32; 0x4],

    tdd_vco_rx_on_2: u32,
    tdd_vco_rx_off_2: u32,
    tdd_vco_tx_on_2: u32,
    tdd_vco_tx_off_2: u32,

    tdd_rx_on_2: u32,
    tdd_rx_off_2: u32,
    tdd_tx_on_2: u32,
    tdd_tx_off_2: u32,

    tdd_rx_dp_on_2: u32,
    tdd_rx_dp_off_2: u32,
    tdd_tx_dp_on_2: u32,
    tdd_tx_dp_off_2: u32,
}

static_assertions::const_assert_eq!(core::mem::offset_of!(Registers, dac), 0x1000);
static_assertions::const_assert_eq!(core::mem::size_of::<Registers>(), 0x20F0);

impl Registers {
    /// Create a new handle to the ADC register block of this IP core.
    ///
    /// # Safety
    ///
    /// See safety notes of [Self::new_mmio].
    pub fn new_adc_block(ip_core_base_addr: usize) -> adc::MmioRegisters<'static> {
        unsafe { adc::Registers::new_mmio_at(ip_core_base_addr + 0x40) }
    }

    /// Create a new handle to the DAC register block of this IP core.
    ///
    /// # Safety
    ///
    /// See safety notes of [Self::new_mmio].
    pub fn new_dac_block(ip_core_base_addr: usize) -> dac::MmioRegisters<'static> {
        unsafe { dac::Registers::new_mmio_at(ip_core_base_addr + 0x1000) }
    }
}
