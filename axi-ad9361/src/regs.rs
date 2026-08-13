pub use fields::InterfaceMode;

/// Register field types shared by the top-level, ADC and DAC register blocks.
pub mod fields {
    use arbitrary_int::u12;

    /// Core reset/clock-enable register, `AXI_ADC_REG_RSTN`/`AXI_DAC_REG_RSTN`.
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

    /// Dynamic Reconfiguration Port (DRP) access request, `AXI_{ADC,DAC}_REG_DRP_CNTRL`. Shared
    /// layout between the ADC and DAC register blocks.
    #[bitbybit::bitfield(
        u32,
        default = 0,
        debug,
        defmt_bitfields(feature = "defmt"),
        forbid_overlaps
    )]
    pub struct DrpControl {
        /// Read (true) or write (false) the addressed DRP register.
        #[bit(28, rw)]
        drp_rwn: bool,
        /// DRP address; designs with multiple DRP primitives select between them here.
        #[bits(16..=27, rw)]
        drp_address: u12,
    }

    /// DRP access status, `AXI_{ADC,DAC}_REG_DRP_STATUS`. Shared layout between the ADC and DAC
    /// register blocks.
    #[bitbybit::bitfield(
        u32,
        default = 0,
        debug,
        defmt_bitfields(feature = "defmt"),
        forbid_overlaps
    )]
    pub struct DrpStatus {
        /// The DRP-driven MMCM/PLL has achieved lock.
        #[bit(17, r)]
        drp_locked: bool,
        /// A DRP access is pending (busy).
        #[bit(16, r)]
        drp_status: bool,
    }

    /// Single-Data-Rate vs. Double-Data-Rate digital interface selection.
    #[bitbybit::bitenum(u1, exhaustive = true)]
    #[derive(Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum InterfaceType {
        /// Single data rate.
        Sdr = 0,
        /// Double data rate.
        Ddr = 1,
    }

    /// Symbol width used by the digital interface.
    #[bitbybit::bitenum(u1, exhaustive = true)]
    #[derive(Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum SymbolModeBits {
        /// 8-bit symbols.
        _8 = 1,
        /// 16-bit symbols.
        _16 = 0,
    }

    /// Number of channels transferred per interface beat.
    #[bitbybit::bitenum(u1, exhaustive = true)]
    #[derive(Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum R1Mode {
        /// One channel per beat (e.g. 1R1T).
        OneChannel = 1,
        /// Two channels per beat (e.g. 2R2T).
        TwoChannels = 0,
    }

    /// Digital interface signaling standard.
    #[bitbybit::bitenum(u1, exhaustive = true)]
    #[derive(Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum InterfaceMode {
        /// LVDS interface.
        Lvds = 0,
        /// CMOS interface.
        Cmos = 1,
    }

    /// IP core capability/configuration register, `AXI_ADC_REG_CONFIG`.
    #[bitbybit::bitfield(
        u32,
        default = 0,
        debug,
        defmt_bitfields(feature = "defmt"),
        forbid_overlaps
    )]
    pub struct Config {
        /// Raw (uncorrected) ADC data can be read back.
        #[bit(11, rw)]
        rd_raw_data: bool,
        /// External synchronization support is present.
        #[bit(10, rw)]
        external_sync: bool,
        /// Only scale correction is supported, not full IQ correction.
        #[bit(9, rw)]
        scale_correction_only: bool,
        /// A PPS receiver is present.
        #[bit(8, rw)]
        pps_receiver: bool,
        /// Digital interface standard configured for this core.
        #[bit(7, rw)]
        cmos_or_lvds: InterfaceMode,
        /// DDS support is disabled.
        #[bit(6, rw)]
        dds_disable: bool,
        /// Delay control support is disabled.
        #[bit(5, rw)]
        delay_control_disable: bool,
        /// Core is built for 1R1T/1T1R mode only.
        #[bit(4, rw)]
        mode_1r1t: bool,

        /// User-port support is disabled.
        #[bit(3, rw)]
        userports_disabled: bool,
        /// Data format conversion support is disabled.
        #[bit(2, rw)]
        dataformat_disabled: bool,
        /// DC filtering support is disabled.
        #[bit(1, r)]
        dc_filter_disabled: bool,
        /// IQ correction support is disabled.
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
        /// Technology not recognized by the encoding.
        Unknown = 0,
        /// Xilinx 7 series.
        Series7 = 1,
        /// Xilinx/AMD UltraScale.
        UltraScale = 2,
        /// Xilinx/AMD UltraScale+.
        UltraScalePlus = 3,
        /// AMD Versal.
        Versal = 4,
    }

    /// FPGA family, encoded by Vivado at build time from the target part.
    ///
    /// See `adi_xilinx_device_info_enc.tcl` (`fpga_family_list`) in the ADI HDL repository.
    #[bitbybit::bitenum(u8, exhaustive = false)]
    #[derive(Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum FpgaFamily {
        /// Family not recognized by the encoding.
        Unknown = 0,
        /// Xilinx Artix.
        Artix = 1,
        /// Xilinx Kintex.
        Kintex = 2,
        /// Xilinx Virtex.
        Virtex = 3,
        /// Xilinx/AMD Zynq.
        Zynq = 4,
        /// AMD Versal Prime.
        VersalPrime = 5,
        /// AMD Versal AI Core.
        VersalAiCore = 6,
        /// AMD Versal Premium.
        VersalPremium = 7,
    }

    /// FPGA part information encoded by Vivado at build time, read from `up_config_info`.
    #[bitbybit::bitfield(
        u32,
        default = 0,
        debug,
        defmt_bitfields(feature = "defmt"),
        forbid_overlaps
    )]
    pub struct FpgaInfo {
        /// FPGA process technology.
        #[bits(24..=31, rw)]
        technology: Option<FpgaTechnology>,
        /// FPGA family.
        #[bits(16..=23, rw)]
        family: Option<FpgaFamily>,
        /// Speed grade, as encoded by Vivado.
        #[bits(8..=15, rw)]
        speed: u8,
        /// Device package, as encoded by Vivado.
        #[bits(0..=7, rw)]
        dev_package: u8,
    }
}

/// ADC register block, `AXI_ADC_REG_*`.
pub mod adc {
    pub use crate::regs::fields::Reset;

    /// ADC-specific register field types.
    pub mod fields {
        use arbitrary_int::{u4, u5};

        pub use crate::regs::fields::{InterfaceType, R1Mode, SymbolModeBits};

        /// DDR data capture edge selection.
        #[bitbybit::bitenum(u1, exhaustive = true)]
        #[derive(Debug, PartialEq, Eq)]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        pub enum DdrEdgeSelect {
            /// Capture on the rising edge.
            Rising = 0,
            /// Capture on the falling edge.
            Falling = 1,
        }

        /// Digital interface pin/clock multiplexing scheme.
        #[bitbybit::bitenum(u1, exhaustive = true)]
        #[derive(Debug, PartialEq, Eq)]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        pub enum PinMode {
            /// Clock-multiplexed interface.
            ClockMultiplexed = 1,
            /// Pin-multiplexed interface.
            PinMultiplexed = 0,
        }

        /// ADC digital interface control register, `AXI_ADC_REG_CNTRL_1`.
        #[bitbybit::bitfield(
            u32,
            default = 0,
            debug,
            defmt_bitfields(feature = "defmt"),
            forbid_overlaps
        )]
        pub struct Control1 {
            /// Interface type (SDR/DDR).
            #[bit(16, rw)]
            interface_type: InterfaceType,
            /// Select symbol data format mode.
            #[bit(15, rw)]
            symb_op: bool,
            /// Symbol width used by the digital interface.
            #[bit(14, rw)]
            symb_8_16b: SymbolModeBits,
            /// Number of active interface lanes.
            #[bits(8..=12, rw)]
            num_of_lanes: u5,
            /// Request a channel data path synchronization pulse.
            #[bit(3, rw)]
            sync: bool,
            /// Number of channels transferred per interface beat.
            #[bit(2, rw)]
            r1_mode: R1Mode,
            /// DDR data capture edge.
            #[bit(1, rw)]
            ddr_edgesel: DdrEdgeSelect,
            /// Pin/clock multiplexing scheme.
            #[bit(0, rw)]
            pin_mode: PinMode,
        }

        /// External-sync trigger control, `AXI_ADC_REG_CNTRL_2`.
        #[bitbybit::bitfield(
            u32,
            default = 0,
            debug,
            defmt_bitfields(feature = "defmt"),
            forbid_overlaps
        )]
        pub struct Control2 {
            /// Issue a manual external synchronization event.
            #[bit(8, rw)]
            manual_sync_request: bool,
            /// Disarm the external synchronization trigger mechanism.
            #[bit(2, rw)]
            ext_sync_disarm: bool,
            /// Arm the external synchronization trigger mechanism.
            #[bit(1, rw)]
            ext_sync_arm: bool,
        }

        /// CRC and output format control, `AXI_ADC_REG_CNTRL_3`.
        #[bitbybit::bitfield(
            u32,
            default = 0,
            debug,
            defmt_bitfields(feature = "defmt"),
            forbid_overlaps
        )]
        pub struct Control3 {
            /// Enable CRC generation.
            #[bit(8, rw)]
            crc_en: bool,
            /// Select the output format decode mode.
            #[bits(0..=7, rw)]
            custom_control: u8,
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
            /// Loop the channel's data path back on itself.
            #[bit(11, rw)]
            loopback_enable: bool,
            /// Legacy single-bit PN select, distinct from [`ChannelPnSelect::pn_sel`].
            #[bit(10, rw)]
            pn_sel_legacy: bool,
            /// Enable IQ correction on this channel.
            #[bit(9, rw)]
            iqcor_enable: bool,
            /// Enable DC filtering on this channel.
            #[bit(8, rw)]
            dcfilt_enable: bool,
            /// Sign-extend the formatted sample.
            #[bit(6, rw)]
            format_signext: bool,
            /// Sample format type (offset binary vs. two's complement).
            #[bit(5, rw)]
            format_type: bool,
            /// Enable sample format conversion.
            #[bit(4, rw)]
            format_enable: bool,
            /// PN sequence type used by the legacy PN monitor.
            #[bit(1, rw)]
            pn_type: bool,
            /// Enable the channel's data path.
            #[bit(0, rw)]
            enable: bool,
        }

        /// Per-channel status register, `AXI_ADC_REG_CHAN_STATUS`.
        #[bitbybit::bitfield(
            u32,
            default = 0,
            debug,
            defmt_bitfields(feature = "defmt"),
            forbid_overlaps
        )]
        pub struct ChannelStatus {
            /// CRC error detected on the channel's samples.
            #[bit(12, rw)]
            crc_err: bool,
            /// Sample status header byte.
            #[bits(4..=11, rw)]
            status_header: u8,
            /// PN sequence mismatch detected.
            #[bit(2, rw)]
            pn_error: bool,
            /// PN sequence checker has lost synchronization.
            #[bit(1, rw)]
            pn_out_of_sync: bool,
            /// Sample value exceeded the representable range.
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
            /// PN9 sequence (the only variant actually decoded, along with [`Self::PnCustom`]).
            Pn9 = 0x0,
            /// Unimplemented on AD9361; behaves like [`Self::Pn9`].
            _Pn23A = 0x1,
            /// Unimplemented on AD9361; behaves like [`Self::Pn9`].
            _Pn7 = 0x4,
            /// Unimplemented on AD9361; behaves like [`Self::Pn9`].
            _Pn15 = 0x5,
            /// Unimplemented on AD9361; behaves like [`Self::Pn9`].
            _Pn23 = 0x6,
            /// Unimplemented on AD9361; behaves like [`Self::Pn9`].
            _Pn31 = 0x7,
            /// Custom PN sequence (decoded distinctly from [`Self::Pn9`]).
            PnCustom = 0x9,
            /// Unimplemented on AD9361; behaves like [`Self::Pn9`].
            _PnRampNibble = 0xA,
            /// Unimplemented on AD9361; behaves like [`Self::Pn9`].
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
            /// Global control status, not documented in the no-OS header.
            #[bit(4, r)]
            ctrl_status: bool,
            /// PN error asserted on any muxed channel.
            #[bit(3, r)]
            mux_pn_error: bool,
            /// PN synchronization lost on any muxed channel.
            #[bit(2, r)]
            mux_pn_out_of_sync: bool,
            /// Over-range condition on any muxed channel.
            #[bit(1, r)]
            mux_over_range: bool,
            /// Interface has achieved lock.
            #[bit(0, r)]
            locked: bool,
        }

        /// Per-channel PN monitor selection register.
        #[bitbybit::bitfield(
            u32,
            default = 0,
            debug,
            defmt_bitfields(feature = "defmt"),
            forbid_overlaps
        )]
        pub struct ChannelPnSelect {
            /// Selected PN sequence.
            #[bits(16..=19, rw)]
            pn_sel: Option<PnSel>,
            /// Data source selection for the PN monitor.
            #[bits(0..=3, rw)]
            data_sel: u4,
        }

        /// IDELAY tap control, `AXI_ADC_REG_DELAY_CNTRL`.
        ///
        /// Deprecated since HDL v9 in favor of [`super::super::fields::DrpControl`]-based delay
        /// access on newer interface primitives, but still present in the register map.
        #[bitbybit::bitfield(
            u32,
            default = 0,
            debug,
            defmt_bitfields(feature = "defmt"),
            forbid_overlaps
        )]
        pub struct DelayControl {
            /// A 0 -> 1 transition on this bit initiates a delay access.
            #[bit(17, rw)]
            delay_sel: bool,
            /// Read (true) or write (false) the addressed delay tap.
            #[bit(16, rw)]
            delay_rwn: bool,
            /// Delay tap address; the valid range depends on the interface pins.
            #[bits(8..=15, rw)]
            delay_address: u8,
            /// Delay write data. A value of 1 corresponds to (1/200) ns.
            #[bits(0..=4, rw)]
            delay_wdata: u5,
        }

        /// IDELAY tap status, `AXI_ADC_REG_DELAY_STATUS`. See [`DelayControl`] for deprecation
        /// notes.
        #[bitbybit::bitfield(
            u32,
            default = 0,
            debug,
            defmt_bitfields(feature = "defmt"),
            forbid_overlaps
        )]
        pub struct DelayStatusLegacy {
            /// The delay tap has locked.
            #[bit(9, r)]
            delay_locked: bool,
            /// A delay access is pending (busy).
            #[bit(8, r)]
            delay_status: bool,
            /// Current delay tap value.
            #[bits(0..=4, r)]
            delay_rdata: u5,
        }

        /// User interface FIFO status, `AXI_ADC_REG_UI_STATUS`. Overflow/underflow bits are
        /// write-1-to-clear.
        #[bitbybit::bitfield(
            u32,
            default = 0,
            debug,
            defmt_bitfields(feature = "defmt"),
            forbid_overlaps
        )]
        pub struct UiStatus {
            /// User interface FIFO overflow occurred.
            #[bit(2, rw)]
            ui_ovf: bool,
            /// User interface FIFO underflow occurred.
            #[bit(1, rw)]
            ui_unf: bool,
        }
    }

    /// Per-channel ADC register block.
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

    /// ADC register block.
    #[derive(derive_mmio::Mmio)]
    #[repr(C)]
    pub struct Registers {
        reset: Reset,
        adc_control1: fields::Control1,
        adc_control2: fields::Control2,
        adc_control3: fields::Control3,
        _gap2: u32,
        adc_clock_freq: u32,
        adc_clock_ratio: u32,
        #[mmio(PureRead)]
        adc_status: fields::Status,
        adc_delay_control: fields::DelayControl,
        #[mmio(PureRead)]
        adc_delay_status_legacy: fields::DelayStatusLegacy,
        #[mmio(PureRead)]
        adc_sync_status: u32,
        _gap3: u32,
        adc_drp_control: crate::regs::fields::DrpControl,
        #[mmio(PureRead)]
        adc_drp_status: crate::regs::fields::DrpStatus,
        adc_drp_wdata: u32,
        #[mmio(PureRead)]
        adc_drp_rdata: u32,
        adc_config_write: u32,
        #[mmio(PureRead)]
        adc_config_read: u32,
        ui_status: fields::UiStatus,
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

/// DAC register block, `AXI_DAC_REG_*`.
pub mod dac {
    use crate::regs::dac::regs::{Control1, Control2, RateControl, UiStatus};
    pub use crate::regs::fields::Reset;

    /// DAC-specific register field types.
    pub mod regs {
        pub use arbitrary_int::u5;

        pub use crate::regs::fields::{InterfaceType, R1Mode, SymbolModeBits};

        /// Frame/parity bit polarity.
        #[bitbybit::bitenum(u1, exhaustive = true)]
        #[derive(Debug, PartialEq, Eq)]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        pub enum ParityType {
            /// Even parity.
            Even = 0,
            /// Odd parity.
            Odd = 1,
        }

        /// Frame/parity bit meaning.
        #[bitbybit::bitenum(u1, exhaustive = true)]
        #[derive(Debug, PartialEq, Eq)]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        pub enum ParityMode {
            /// Bit carries frame information.
            Frame = 0,
            /// Bit carries parity information.
            Parity = 1,
        }

        /// DAC channel synchronization control register, `AXI_DAC_REG_CNTRL_1`.
        #[bitbybit::bitfield(
            u32,
            default = 0,
            debug,
            defmt_bitfields(feature = "defmt"),
            forbid_overlaps
        )]
        pub struct Control1 {
            /// Manually request a synchronization pulse.
            #[bit(8, rw)]
            manual_sync_request: bool,
            /// Disarm external synchronization.
            #[bit(2, rw)]
            disarm_ext_sync: bool,
            /// Arm external synchronization.
            #[bit(1, rw)]
            arm_ext_sync: bool,
            /// Request a channel data path synchronization pulse.
            #[bit(0, rw)]
            sync: bool,
        }

        /// DAC digital interface control register, `AXI_DAC_REG_CNTRL_2`.
        #[bitbybit::bitfield(
            u32,
            default = 0,
            debug,
            defmt_bitfields(feature = "defmt"),
            forbid_overlaps
        )]
        pub struct Control2 {
            /// Interface type (SDR/DDR).
            #[bit(16, rw)]
            interface_type: InterfaceType,
            /// Select symbol data format mode.
            #[bit(15, rw)]
            symb_op: bool,
            /// Symbol width used by the digital interface.
            #[bit(14, rw)]
            symb_8_16b: SymbolModeBits,
            /// Number of active interface lanes.
            #[bits(8..=12, rw)]
            num_of_lanes: u5,
            /// Frame/parity bit polarity.
            #[bit(7, rw)]
            parity_type: ParityType,
            /// Frame/parity bit meaning.
            #[bit(6, rw)]
            parity_mode: ParityType,
            /// Number of channels transferred per interface beat.
            #[bit(5, rw)]
            r1_mode: R1Mode,
            /// Sample data format (offset binary vs. two's complement).
            #[bit(4, rw)]
            data_format: bool,
        }

        /// DAC sample rate divider register, `AXI_DAC_REG_RATECNTRL`.
        #[bitbybit::bitfield(
            u32,
            default = 0,
            debug,
            defmt_bitfields(feature = "defmt"),
            forbid_overlaps
        )]
        pub struct RateControl {
            /// Rate divider value, see [`crate::dac::Dac::set_rate_div`].
            #[bits(0..=7, rw)]
            rate: u8,
        }

        /// User interface FIFO status, `AXI_DAC_REG_UI_STATUS`. Overflow/underflow bits are
        /// write-1-to-clear.
        #[bitbybit::bitfield(
            u32,
            default = 0,
            debug,
            defmt_bitfields(feature = "defmt"),
            forbid_overlaps
        )]
        pub struct UiStatus {
            /// The data interface is busy.
            #[bit(4, r)]
            if_busy: bool,
            /// User interface FIFO overflow occurred.
            #[bit(1, rw)]
            ui_ovf: bool,
            /// User interface FIFO underflow occurred.
            #[bit(0, rw)]
            ui_unf: bool,
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
            /// Internally generated tone (DDS).
            InternalTone = 0x00,
            /// [`PatternSed`] test pattern.
            Pattern = 0x01,
            /// Data supplied by software/user logic.
            InputData = 0x02,
            /// Constant zero output.
            Zero = 0x03,
            /// Not matched by an explicit arm; falls through to [`Self::InternalTone`].
            _InvertedPn7 = 0x04,
            /// Not matched by an explicit arm; falls through to [`Self::InternalTone`].
            _InvertedPn15 = 0x05,
            /// Not matched by an explicit arm; falls through to [`Self::InternalTone`].
            _Pn7 = 0x06,
            /// Not matched by an explicit arm; falls through to [`Self::InternalTone`].
            _Pn15 = 0x07,
            /// Loop the corresponding ADC channel's data back out on this DAC channel.
            LoopbackDataAdc = 0x08,
            /// PN sequence output.
            PnX = 0x09,
            /// Not matched by an explicit arm; falls through to [`Self::InternalTone`].
            _NibbleRamp = 0x0A,
            /// Not matched by an explicit arm; falls through to [`Self::InternalTone`].
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
            /// Enable IQ correction on this channel.
            #[bit(2, rw)]
            iqcor_enable: bool,
            /// Loop the channel's data path back on itself.
            #[bit(1, rw)]
            loopback_enable: bool,
            /// Enable PN sequence output on this channel.
            #[bit(0, rw)]
            pn_enable: bool,
        }

        /// Per-channel data source mux register, `AXI_DAC_REG_CHAN_CNTRL_7`.
        #[bitbybit::bitfield(
            u32,
            default = 0,
            debug,
            defmt_bitfields(feature = "defmt"),
            forbid_overlaps
        )]
        pub struct ChannelDataSource {
            /// Selected data source.
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
            /// Second alternating pattern value.
            #[bits(16..=31, rw)]
            pattern_2: i16,
            /// First alternating pattern value.
            #[bits(0..=15, rw)]
            pattern_1: i16,
        }
    }

    /// Per-channel DAC register block.
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

    /// DAC register block.
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
        dac_drp_control: crate::regs::fields::DrpControl,
        #[mmio(PureRead)]
        dac_drp_status: crate::regs::fields::DrpStatus,
        dac_drp_wdata: u32,
        dac_drp_rdata: u32,
        dac_custom_read: u32,
        dac_custom_write: u32,
        dac_ui_status: UiStatus,
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

/// Top-level AXI AD9361 IP core register block, spanning the shared registers plus the ADC,
/// DAC and TDD sub-blocks.
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
