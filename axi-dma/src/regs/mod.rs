//! Register definitions for the AXI DMA IP core.

/// Direct-register (simple) mode register definitions.
pub mod direct_register;
/// Scatter-gather mode register and descriptor definitions.
pub mod scatter_gather;

/// Bitfield types shared by [`direct_register`] and [`scatter_gather`].
pub mod fields {
    /// DMA channel run/stop control bit value.
    #[bitbybit::bitenum(u1, exhaustive = true)]
    #[derive(Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum RunStop {
        /// Stop the DMA channel.
        Stop = 0,
        /// Run the DMA channel.
        Run = 1,
    }

    /// MM2S/S2MM control register (DMACR).
    #[bitbybit::bitfield(
        u32,
        default = 0,
        debug,
        defmt_bitfields(feature = "defmt"),
        forbid_overlaps
    )]
    pub struct Control {
        /// Interrupt delay timeout count.
        #[bits(24..=31, rw)]
        interrupt_delay: u8,
        /// Number of completed transfers before an interrupt-on-complete fires.
        #[bits(16..=23, rw)]
        interrupt_threshold: u8,
        /// Enables the error interrupt.
        #[bit(14, rw)]
        error_interrupt_enable: bool,
        /// Enables the delay timer interrupt.
        #[bit(13, rw)]
        delay_timer_interrupt_enable: bool,
        /// Enables the completion (IOC) interrupt.
        #[bit(12, rw)]
        interrupt_on_complete: bool,
        /// Enables cyclic buffer descriptor mode (scatter-gather only).
        #[bit(4, rw)]
        cyclic_bd_enable: bool,
        /// Enables keyhole addressing.
        #[bit(3, rw)]
        keyhole: bool,
        /// Resets the DMA/SG engine. Self-clearing once the reset completes.
        #[bit(2, rw)]
        reset: bool,
        /// Starts or stops the channel.
        #[bit(0, rw)]
        run_stop: RunStop,
    }

    /// MM2S/S2MM status register (DMASR).
    #[bitbybit::bitfield(
        u32,
        default = 0,
        debug,
        defmt_bitfields(feature = "defmt"),
        forbid_overlaps
    )]
    pub struct Status {
        /// Current interrupt delay timeout count.
        #[bits(24..=31, r)]
        interrupt_delay: u8,
        /// Current interrupt threshold count.
        #[bits(16..=23, r)]
        interrupt_threshold: u8,
        /// Write-to-clear interrupt status bit.
        #[bit(14, rw)]
        error_interrupt: bool,
        /// Write-to-clear interrupt status bit.
        #[bit(13, rw)]
        delay_timer_interrupt: bool,
        /// Write-to-clear interrupt status bit.
        #[bit(12, rw)]
        completion_interrupt: bool,
        /// Scatter-gather descriptor decode error.
        #[bit(10, r)]
        sg_decode_error: bool,
        /// Scatter-gather slave (AXI) error.
        #[bit(9, r)]
        sg_slave_error: bool,
        /// Scatter-gather internal error.
        #[bit(8, r)]
        sg_internal_error: bool,
        /// DMA/data stream decode error.
        #[bit(6, r)]
        dma_decode_error: bool,
        /// DMA slave (AXI) error.
        #[bit(5, r)]
        dma_slave_error: bool,
        /// DMA internal error.
        #[bit(4, r)]
        dma_internal_error: bool,
        /// Whether scatter-gather mode is enabled for this channel.
        #[bit(3, r)]
        scatter_gather_enabled: bool,
        /// Channel is idle (no transfer in progress).
        #[bit(1, r)]
        idle: bool,
        /// Channel is halted.
        #[bit(0, r)]
        halted: bool,
    }
}
