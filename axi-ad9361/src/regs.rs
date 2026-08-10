pub use fields::InterfaceMode;

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

    impl Reset {
        /// Both core reset lines released, clock enabled — the value `Adc`/`Dac` write to
        /// bring the core out of reset (second write of `enable()`, and the only write of
        /// `Adc::reset_pulse()`/`Dac::release_reset()`).
        pub const RELEASED: Self = Self::ZERO.with_mmcm_reset_n(true).with_reset_n(true);
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

    #[bitbybit::bitenum(u1, exhaustive = true)]
    #[derive(Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum InterfaceMode {
        Lvds = 0,
        Cmos = 1,
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
        cmos_or_lvds: InterfaceMode,
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

    /// FPGA process technology, encoded by Vivado at build time from the target part.
    ///
    /// See `adi_xilinx_device_info_enc.tcl` (`fpga_technology_list`) in the ADI HDL repository.
    #[bitbybit::bitenum(u8, exhaustive = false)]
    #[derive(Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum FpgaTechnology {
        Unknown = 0,
        Series7 = 1,
        UltraScale = 2,
        UltraScalePlus = 3,
        Versal = 4,
    }

    /// FPGA family, encoded by Vivado at build time from the target part.
    ///
    /// See `adi_xilinx_device_info_enc.tcl` (`fpga_family_list`) in the ADI HDL repository.
    #[bitbybit::bitenum(u8, exhaustive = false)]
    #[derive(Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum FpgaFamily {
        Unknown = 0,
        Artix = 1,
        Kintex = 2,
        Virtex = 3,
        Zynq = 4,
        VersalPrime = 5,
        VersalAiCore = 6,
        VersalPremium = 7,
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
        technology: Option<FpgaTechnology>,
        #[bits(16..=23, rw)]
        family: Option<FpgaFamily>,
        #[bits(8..=15, rw)]
        speed: u8,
        #[bits(0..=7, rw)]
        dev_package: u8,
    }
}

pub mod adc {
    pub use crate::regs::fields::Reset;

    pub mod fields {
        use arbitrary_int::{u4, u5};

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

        /// Per-channel enable/format control, `AXI_ADC_REG_CHAN_CNTRL`.
        ///
        /// `loopback_enable` (bit 11) is not documented in the no-OS `axi_adc_core.h` header,
        /// only present in `up_adc_channel.v`.
        #[bitbybit::bitfield(
            u32,
            default = 0,
            debug,
            defmt_bitfields(feature = "defmt"),
            forbid_overlaps
        )]
        pub struct ChannelDataPathControl {
            #[bit(11, rw)]
            loopback_enable: bool,
            /// Legacy single-bit PN select, distinct from [`ChannelPnSelect::pn_sel`].
            #[bit(10, rw)]
            pn_sel_legacy: bool,
            #[bit(9, rw)]
            iqcor_enable: bool,
            #[bit(8, rw)]
            dcfilt_enable: bool,
            #[bit(6, rw)]
            format_signext: bool,
            #[bit(5, rw)]
            format_type: bool,
            #[bit(4, rw)]
            format_enable: bool,
            #[bit(1, rw)]
            pn_type: bool,
            #[bit(0, rw)]
            enable: bool,
        }

        #[bitbybit::bitfield(
            u32,
            default = 0,
            debug,
            defmt_bitfields(feature = "defmt"),
            forbid_overlaps
        )]
        pub struct ChannelStatus {
            #[bit(12, rw)]
            crc_err: bool,
            #[bits(4..=11, rw)]
            status_header: u8,
            #[bit(2, rw)]
            pn_error: bool,
            #[bit(1, rw)]
            pn_out_of_sync: bool,
            #[bit(0, rw)]
            over_range: bool,
        }

        /// PN sequence selection, written to a channel's [`ChannelPnSelect`] register.
        ///
        /// Per `axi_ad9361_rx_pnmon.v`, only `== 0x9` vs `!= 0x9` is actually decoded. The
        /// underscore-prefixed variants are unimplemented on AD9361 and behave like [`Self::Pn9`].
        #[bitbybit::bitenum(u4, exhaustive = false)]
        #[derive(Debug, PartialEq, Eq)]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        pub enum PnSel {
            Pn9 = 0x0,
            _Pn23A = 0x1,
            _Pn7 = 0x4,
            _Pn15 = 0x5,
            _Pn23 = 0x6,
            _Pn31 = 0x7,
            PnCustom = 0x9,
            _PnRampNibble = 0xA,
            _PnRamp16 = 0xB,
        }

        /// Global PN/interface status, read from `adc_status` (`AXI_ADC_REG_STATUS`).
        ///
        /// `ctrl_status` (bit 4) is not documented in the no-OS `axi_adc_core.h` header, only
        /// present in `up_adc_common.v`.
        #[bitbybit::bitfield(
            u32,
            default = 0,
            debug,
            defmt_bitfields(feature = "defmt"),
            forbid_overlaps
        )]
        pub struct Status {
            #[bit(4, r)]
            ctrl_status: bool,
            #[bit(3, r)]
            mux_pn_error: bool,
            #[bit(2, r)]
            mux_pn_out_of_sync: bool,
            #[bit(1, r)]
            mux_over_range: bool,
            #[bit(0, r)]
            locked: bool,
        }

        #[bitbybit::bitfield(
            u32,
            default = 0,
            debug,
            defmt_bitfields(feature = "defmt"),
            forbid_overlaps
        )]
        pub struct ChannelPnSelect {
            #[bits(16..=19, rw)]
            pn_sel: Option<PnSel>,
            #[bits(0..=3, rw)]
            data_sel: u4,
        }
    }

    #[derive(derive_mmio::Mmio)]
    #[repr(C)]
    pub struct Channel {
        data_path_control: fields::ChannelDataPathControl,
        status: fields::ChannelStatus,
        raw_data: u32,
        _gap0: u32,
        control1: u32,
        control2: u32,
        pn_select: fields::ChannelPnSelect,
        _gap1: u32,
        user_control1: u32,
        user_control2: u32,
        control4: u32,
        _gap2: [u32; 0x05],
    }

    static_assertions::const_assert_eq!(core::mem::size_of::<Channel>(), 0x40);

    #[derive(derive_mmio::Mmio)]
    #[repr(C)]
    pub struct Registers {
        reset: Reset,
        adc_control1: fields::Control1,
        adc_control2: u32,
        adc_control3: u32,
        _gap2: u32,
        adc_clock_freq: u32,
        adc_clock_ratio: u32,
        #[mmio(PureRead)]
        adc_status: fields::Status,
        adc_delay_control: u32,
        #[mmio(PureRead)]
        adc_delay_status_legacy: u32,
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

    static_assertions::const_assert_eq!(core::mem::size_of::<Registers>(), 0x800 - 0x40);
    static_assertions::const_assert_eq!(
        core::mem::offset_of!(Registers, adc_channels),
        0x400 - 0x40
    );
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

        /// Data source mux, written to a channel's [`ChannelDataSource`] register.
        ///
        /// Per the `case (dac_data_sel_s)` mux in `axi_ad9361_tx_channel.v`, only 0x1, 0x2,
        /// 0x3, 0x8 and 0x9 are matched by an explicit arm. The underscore-prefixed variants
        /// fall through to `default` and behave like [`Self::InternalTone`].
        #[bitbybit::bitenum(u4, exhaustive = false)]
        #[derive(Debug, PartialEq, Eq)]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        pub enum DataSource {
            InternalTone = 0x00,
            Pattern = 0x01,
            InputData = 0x02,
            Zero = 0x03,
            _InvertedPn7 = 0x04,
            _InvertedPn15 = 0x05,
            _Pn7 = 0x06,
            _Pn15 = 0x07,
            LoopbackDataAdc = 0x08,
            PnX = 0x09,
            _NibbleRamp = 0x0A,
            _BitRam16Bit = 0x0B,
        }

        /// Legacy per-channel loopback/PN/IQ-correction enable, `control6`.
        ///
        /// Superseded on modern cores by [`ChannelDataSource::data_source`]
        /// (`DataSource::LoopbackDataAdc`/`DataSource::PnX`), but the AD9361 TX digital-tune
        /// sequence still clears and restores this register unconditionally.
        #[bitbybit::bitfield(
            u32,
            default = 0,
            debug,
            defmt_bitfields(feature = "defmt"),
            forbid_overlaps
        )]
        pub struct ChannelLegacyControl {
            #[bit(2, rw)]
            iqcor_enable: bool,
            #[bit(1, rw)]
            loopback_enable: bool,
            #[bit(0, rw)]
            pn_enable: bool,
        }

        #[bitbybit::bitfield(
            u32,
            default = 0,
            debug,
            defmt_bitfields(feature = "defmt"),
            forbid_overlaps
        )]
        pub struct ChannelDataSource {
            #[bits(0..=3, rw)]
            data_source: Option<DataSource>,
        }

        /// SED test pattern data.
        ///
        /// `pattern_1` and `pattern_2` are output alternately, sample by sample, when the
        /// channel's data source is set to [`DataSource::Pattern`].
        #[bitbybit::bitfield(
            u32,
            default = 0,
            debug,
            defmt_bitfields(feature = "defmt"),
            forbid_overlaps
        )]
        pub struct PatternSed {
            #[bits(16..=31, rw)]
            pattern_2: i16,
            #[bits(0..=15, rw)]
            pattern_1: i16,
        }
    }

    #[derive(derive_mmio::Mmio)]
    #[repr(C)]
    pub struct Channel {
        control1: u32,
        control2: u32,
        control3: u32,
        control4: u32,
        pattern_sed: regs::PatternSed,
        legacy_control: regs::ChannelLegacyControl,
        data_source: regs::ChannelDataSource,
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
        interface_clock_ratio: u32,
        /// Bit 0: interface status. Set if there are no errors; if unset, there are errors and
        /// software may try resetting the core.
        #[mmio(PureRead)]
        interface_status: u32,
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

    static_assertions::const_assert_eq!(core::mem::size_of::<Registers>(), 0x800);
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

    _gap7: [u32; 0xE00],

    #[mmio(Inner)]
    dac: dac::Registers,

    _gap14: [u32; 0xE00],

    // TDD registers.
    _tdd_start: [u32; 0x10],
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

static_assertions::const_assert_eq!(core::mem::offset_of!(Registers, adc), 0x40);
static_assertions::const_assert_eq!(core::mem::offset_of!(Registers, dac), 0x4000);
static_assertions::const_assert_eq!(core::mem::offset_of!(Registers, _tdd_start), 0x8000);

impl Registers {
    /// Create a new handle to the ADC register block of this IP core.
    ///
    /// # Safety
    ///
    /// See safety notes of [Self::new_mmio].
    pub const fn new_adc_block(ip_core_base_addr: usize) -> adc::MmioRegisters<'static> {
        unsafe { adc::Registers::new_mmio_at(ip_core_base_addr + 0x40) }
    }

    /// Create a new handle to the DAC register block of this IP core.
    ///
    /// # Safety
    ///
    /// See safety notes of [Self::new_mmio].
    pub const fn new_dac_block(ip_core_base_addr: usize) -> dac::MmioRegisters<'static> {
        unsafe { dac::Registers::new_mmio_at(ip_core_base_addr + 0x4000) }
    }
}
