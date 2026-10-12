//! The bytes of every write the driver sends, as pure functions: one source of truth shared by
//! the blocking [`Is31fl3236`](crate::Is31fl3236) and the async
//! [`Is31fl3236Async`](crate::asynch::Is31fl3236Async), so the two cannot drift apart. Each
//! function returns exactly the payload of one I2C write (register address first, then data);
//! the drivers add nothing. Register facts and citations are in [`reg`](crate::reg), in
//! [`variant`](crate::variant) and on the driver methods that call these.

use crate::variant::Variant;
use crate::{Error, LedControl, reg};

/// The longest write: one register address byte and one data byte per output of the largest
/// variant (36). At 37 bytes it lives on the stack; the driver never allocates.
pub(crate) const MAX_BLOCK_LEN: usize = 1 + 36;

/// A block write: the register address of the first output, then one byte per output, sent
/// with address auto-increment (DS "Address Auto Increment" and Figure 5, p. 7-8).
pub(crate) struct Block {
    bytes: [u8; MAX_BLOCK_LEN],
    len: usize,
}

impl Block {
    /// The bytes to send: `1 + V::CHANNELS` of them.
    pub(crate) fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

/// Shutdown Register: SSD = 1 leaves software shutdown, 0 enters it (DS Table 3, p. 8). The
/// other seven bits are unused and default to 0, so the byte is `0x01` or `0x00`.
pub(crate) const fn shutdown(enabled: bool) -> [u8; 2] {
    [reg::SHUTDOWN, if enabled { reg::SHUTDOWN_SSD } else { 0 }]
}

/// Reset Register: `00h` resets every register to its default (DS "4Fh Reset Register", p. 9).
pub(crate) const fn reset() -> [u8; 2] {
    [reg::RESET, reg::RESET_VALUE]
}

/// Update Register: `00h` latches everything staged (DS "25h PWM Update Register", p. 9).
pub(crate) const fn update() -> [u8; 2] {
    [reg::UPDATE, reg::UPDATE_LATCH]
}

/// Global Control Register: G_EN = 0 for normal operation, 1 to shut down all LEDs (DS Table 6,
/// p. 9). Bits D7:D1 are unused and written as 0, their default.
pub(crate) const fn global_control(leds_enabled: bool) -> [u8; 2] {
    [
        reg::GLOBAL_CONTROL,
        if leds_enabled {
            0
        } else {
            reg::GLOBAL_CONTROL_G_EN
        },
    ]
}

/// Output Frequency Setting Register (`4Bh`, "A" parts only): OFS in bit D0 (IS31FL3236A DS
/// Table 7, p. 9). Bits D7:D1 are unused and written as 0, their default.
pub(crate) const fn output_frequency(frequency: reg::OutputFrequency) -> [u8; 2] {
    [reg::OUTPUT_FREQUENCY, frequency as u8]
}

/// One channel's PWM register with `value` (DS Table 4, p. 8), or the error for a channel the
/// chip does not have.
pub(crate) fn pwm<V: Variant, E>(channel: usize, value: u8) -> Result<[u8; 2], Error<E>> {
    Ok([channel_register::<V, E>(V::PWM_FIRST, channel)?, value])
}

/// One channel's LED Control register (DS Table 5, p. 9), or the error for a channel the chip
/// does not have.
pub(crate) fn led_control<V: Variant, E>(
    channel: usize,
    control: LedControl,
) -> Result<[u8; 2], Error<E>> {
    Ok([
        channel_register::<V, E>(V::LED_CONTROL_FIRST, channel)?,
        control.bits(),
    ])
}

/// The auto-increment write of all PWM registers: OUT1's register, then element `n` of `frame`
/// for channel `n`. The update is a separate write ([`update`]); see `Is31fl3236::set_frame` for
/// why.
pub(crate) fn frame_block<V: Variant>(frame: &V::Frame) -> Block {
    let frame = frame.as_ref();
    // `Frame` is `[u8; V::CHANNELS]` for every variant, so this holds by construction.
    debug_assert_eq!(frame.len(), V::CHANNELS);
    let mut bytes = [0u8; MAX_BLOCK_LEN];
    bytes[0] = V::PWM_FIRST;
    bytes[1..=V::CHANNELS].copy_from_slice(frame);
    Block {
        bytes,
        len: 1 + V::CHANNELS,
    }
}

/// The auto-increment write that gives all LED Control registers the same value: OUT1's
/// register, then one copy of `control`'s bits per output (DS "Address Auto Increment", p. 7;
/// Table 5, p. 9).
pub(crate) fn led_controls_block<V: Variant>(control: LedControl) -> Block {
    let mut bytes = [control.bits(); MAX_BLOCK_LEN];
    bytes[0] = V::LED_CONTROL_FIRST;
    Block {
        bytes,
        len: 1 + V::CHANNELS,
    }
}

/// The register of `channel` in a per-output range that starts at `first`, or the error for a
/// channel the chip does not have.
///
/// Checking here keeps a wrong index from reaching a neighbouring register: on the IS31FL3236,
/// PWM channel 36 would be the Update Register, and LED Control channel 36 the Global Control
/// Register (DS Table 2, p. 8).
fn channel_register<V: Variant, E>(first: u8, channel: usize) -> Result<u8, Error<E>> {
    match u8::try_from(channel) {
        Ok(offset) if channel < V::CHANNELS => Ok(first + offset),
        _ => Err(Error::ChannelOutOfRange),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::variant::{Is31fl3235, Is31fl3235A, Is31fl3236, Is31fl3236A};
    use crate::{OutputCurrent, OutputFrequency};

    type Bytes2 = Result<[u8; 2], Error<()>>;

    const QUARTER_ON: LedControl = LedControl {
        on: true,
        current: OutputCurrent::Quarter,
    };
    const HALF_OFF: LedControl = LedControl {
        on: false,
        current: OutputCurrent::Half,
    };

    #[test]
    fn single_register_writes_match_the_datasheet() {
        assert_eq!(shutdown(true), [0x00, 0x01]);
        assert_eq!(shutdown(false), [0x00, 0x00]);
        assert_eq!(reset(), [0x4f, 0x00]);
        assert_eq!(update(), [0x25, 0x00]);
        // G_EN = 1 shuts all LEDs down, so "enabled" is 0.
        assert_eq!(global_control(true), [0x4a, 0x00]);
        assert_eq!(global_control(false), [0x4a, 0x01]);
        assert_eq!(output_frequency(OutputFrequency::Khz3), [0x4b, 0x00]);
        assert_eq!(output_frequency(OutputFrequency::Khz22), [0x4b, 0x01]);
    }

    #[test]
    fn channel_registers_span_01h_to_24h_and_26h_to_49h_and_refuse_36() {
        assert_eq!(pwm::<Is31fl3236, _>(0, 0x10) as Bytes2, Ok([0x01, 0x10]));
        assert_eq!(pwm::<Is31fl3236, _>(35, 0xff) as Bytes2, Ok([0x24, 0xff]));
        assert_eq!(
            pwm::<Is31fl3236, _>(36, 0xff) as Bytes2,
            Err(Error::ChannelOutOfRange)
        );
        assert_eq!(
            led_control::<Is31fl3236, _>(0, QUARTER_ON) as Bytes2,
            Ok([0x26, 0x07])
        );
        assert_eq!(
            led_control::<Is31fl3236, _>(35, HALF_OFF) as Bytes2,
            Ok([0x49, 0x02])
        );
        assert_eq!(
            led_control::<Is31fl3236, _>(36, QUARTER_ON) as Bytes2,
            Err(Error::ChannelOutOfRange)
        );
        assert_eq!(
            led_control::<Is31fl3236, _>(usize::MAX, QUARTER_ON) as Bytes2,
            Err(Error::ChannelOutOfRange)
        );
        // The IS31FL3236A has the same map.
        assert_eq!(pwm::<Is31fl3236A, _>(35, 0xff) as Bytes2, Ok([0x24, 0xff]));
        assert_eq!(
            led_control::<Is31fl3236A, _>(0, QUARTER_ON) as Bytes2,
            Ok([0x26, 0x07])
        );
    }

    /// The IS31FL3235 parts: OUT1 at 05h and 2Ah, OUT28 at 20h and 45h, nothing beyond.
    #[test]
    fn the_3235_ranges_start_at_05h_and_2ah_and_refuse_28() {
        for_3235::<Is31fl3235>();
        for_3235::<Is31fl3235A>();
    }

    fn for_3235<V: Variant>() {
        assert_eq!(pwm::<V, _>(0, 0x10) as Bytes2, Ok([0x05, 0x10]));
        assert_eq!(pwm::<V, _>(27, 0xff) as Bytes2, Ok([0x20, 0xff]));
        assert_eq!(
            pwm::<V, _>(28, 0xff) as Bytes2,
            Err(Error::ChannelOutOfRange)
        );
        assert_eq!(
            led_control::<V, _>(0, QUARTER_ON) as Bytes2,
            Ok([0x2a, 0x07])
        );
        assert_eq!(
            led_control::<V, _>(27, HALF_OFF) as Bytes2,
            Ok([0x45, 0x02])
        );
        assert_eq!(
            led_control::<V, _>(28, QUARTER_ON) as Bytes2,
            Err(Error::ChannelOutOfRange)
        );
    }

    #[test]
    fn blocks_start_at_the_first_register_and_carry_one_value_per_output_in_order() {
        let frame: [u8; 36] = core::array::from_fn(|channel| 0xa0 + channel as u8);
        let block = frame_block::<Is31fl3236>(&frame);
        assert_eq!(block.as_slice().len(), 37);
        assert_eq!(block.as_slice()[0], 0x01);
        assert_eq!(&block.as_slice()[1..], &frame);
        let block = led_controls_block::<Is31fl3236>(QUARTER_ON);
        assert_eq!(block.as_slice().len(), 37);
        assert_eq!(block.as_slice()[0], 0x26);
        assert!(block.as_slice()[1..].iter().all(|&b| b == 0x07));

        let frame: [u8; 28] = core::array::from_fn(|channel| 0xa0 + channel as u8);
        let block = frame_block::<Is31fl3235A>(&frame);
        assert_eq!(block.as_slice().len(), 29);
        assert_eq!(block.as_slice()[0], 0x05);
        assert_eq!(&block.as_slice()[1..], &frame);
        let block = led_controls_block::<Is31fl3235A>(QUARTER_ON);
        assert_eq!(block.as_slice().len(), 29);
        assert_eq!(block.as_slice()[0], 0x2a);
    }
}
