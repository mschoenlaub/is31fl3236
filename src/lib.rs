#![doc = include_str!("../README.md")]
//!
//! ## Registers
//!
//! | Address | Register | Driver |
//! | --- | --- | --- |
//! | `00h` | Shutdown (SSD, bit 0: 1 = normal operation) | [`enable`](Is31fl3236::enable), [`shutdown`](Is31fl3236::shutdown) |
//! | PWM range | one PWM value per output | [`set_pwm`](Is31fl3236::set_pwm), [`set_frame`](Is31fl3236::set_frame) |
//! | `25h` | Update (write `00h` to latch PWM and LED Control) | [`update`](Is31fl3236::update), and inside `set_frame` and `set_all_led_controls` |
//! | LED Control range | per output: OUT (bit 0) and the current SL (bits 2:1) | [`set_led_control`](Is31fl3236::set_led_control), [`set_all_led_controls`](Is31fl3236::set_all_led_controls) |
//! | `4Ah` | Global Control (G_EN, bit 0: 1 = all LEDs off) | [`set_global_led_enable`](Is31fl3236::set_global_led_enable) |
//! | `4Bh` | Output Frequency ("A" parts only) | [`set_output_frequency`](Is31fl3236::set_output_frequency) |
//! | `4Fh` | Reset (write `00h`) | [`reset`](Is31fl3236::reset) |
//!
//! The ranges per chip are in [`variant`]; the constants with their datasheet citations in
//! [`reg`]. "DS" in this crate is the IS31FL3236 datasheet, Rev. H.
//!
//! ## How writes take effect
//!
//! PWM and LED Control writes go to temporary registers and take effect together on a write of
//! `00h` to the Update Register (DS "25h PWM Update Register", p. 9). The chip powers up, and comes
//! out of a reset, in software shutdown with every output off (DS Table 2), and its registers keep
//! their data through software and hardware shutdown (DS "Shutdown Mode", p. 12). The register
//! address auto-increments within one write (DS "Address Auto Increment", p. 7), so consecutive
//! registers go out in one I2C write; the datasheet limits SCL to 400 kHz (DS p. 6).
//!
//! ## No register readback
//!
//! "Since IS31FL3236 only supports write operations, A0 must always be '0'" (DS "I2C Interface",
//! p. 7). The driver cannot discover the chip's state, so it starts from a reset, and nobody can
//! save and restore the registers: restoring means writing a known state again. The only thing
//! the bus reveals is whether the address acknowledges.
#![cfg_attr(not(test), no_std)]

use core::marker::PhantomData;

use embedded_hal::i2c::I2c;

#[cfg(feature = "async")]
pub mod asynch;
pub mod reg;
pub mod variant;
mod wire;

pub use reg::{OutputCurrent, OutputFrequency};
use variant::{SelectableFrequency, Variant};

/// Outputs of the IS31FL3236: "36 constant current channels each with independent PWM control"
/// (DS "General Description", p. 1), OUT1..=OUT36. Other variants: [`Variant::CHANNELS`].
pub const CHANNELS: usize = 36;

/// What one LED Control register holds: whether the output is on, and its share of the maximum
/// current (DS Table 5 "26h~49h LED Control Register (OUT1~OUT36)", p. 9).
///
/// After power-up or a reset every output is off at full current, `LedControl { on: false,
/// current: OutputCurrent::Full }` (DS Table 5, default `00000 00 0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LedControl {
    /// The OUT bit: `true` is LED on. An output that is off stays dark whatever its PWM value;
    /// the PWM register keeps the value.
    pub on: bool,
    /// The SL field: the output's current as a fraction of `IMAX`, which the resistor on the
    /// chip's R_EXT pin fixes for all outputs ([`OutputCurrent`]).
    pub current: OutputCurrent,
}

impl LedControl {
    /// The register value: SL in bits D2:D1 and OUT in bit D0 (DS Table 5, p. 9). Bits D7:D3 are
    /// unused and written as 0, their default.
    const fn bits(self) -> u8 {
        let out = if self.on { reg::LED_CONTROL_OUT } else { 0 };
        ((self.current as u8) << reg::LED_CONTROL_SL_SHIFT) | out
    }
}

/// Errors from the driver. The chip cannot be read and so has nothing to report on its own (DS
/// "I2C Interface", p. 7): what can go wrong is the bus, or an argument.
///
/// `Clone` and `Copy` follow the bus error's (embedded-hal's `ErrorKind` and most HALs' errors
/// are `Copy`), so a caller can keep the error and still report it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error<E> {
    /// The I2C transaction failed.
    Bus(E),
    /// A channel argument was not below the variant's [`Variant::CHANNELS`]. Nothing was sent to
    /// the chip.
    ChannelOutOfRange,
}

/// Names the failure; a bus error is shown with its `Debug` form, the one representation every
/// `embedded-hal` bus error has (`embedded_hal::i2c::Error` requires `Debug`, not `Display`).
impl<E: core::fmt::Debug> core::fmt::Display for Error<E> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Bus(error) => write!(f, "IS31FL3236 I2C error: {error:?}"),
            Self::ChannelOutOfRange => write!(f, "IS31FL3236 channel out of range"),
        }
    }
}

impl<E: core::fmt::Debug> core::error::Error for Error<E> {}

/// An IS31FL3236, or a register-compatible variant `V` ([`variant`]), on an `embedded-hal` I2C
/// bus.
///
/// The driver holds no copy of the chip's registers: every method turns into the I2C writes its
/// documentation names and nothing else. See the crate documentation, "Using the driver", for the
/// order of calls and for which methods send the update write themselves.
pub struct Is31fl3236<I2C, V = variant::Is31fl3236> {
    i2c: I2C,
    address: u8,
    variant: PhantomData<V>,
}

impl<I2C: I2c> Is31fl3236<I2C> {
    /// An IS31FL3236 at `address`, one of the four the AD pin selects ([`reg::ADDRESS_AD_GND`] to
    /// [`reg::ADDRESS_AD_VCC`]). Talks to nothing until a method is called. For another chip of
    /// the family, use [`new_variant`](Self::new_variant).
    pub fn new(i2c: I2C, address: u8) -> Self {
        Self::new_variant(i2c, address)
    }
}

impl<I2C: I2c, V: Variant> Is31fl3236<I2C, V> {
    /// The chip `V` at `address` (the same four addresses on every variant), for example
    /// `Is31fl3236::<_, variant::Is31fl3235A>::new_variant(i2c, address)`. Talks to nothing until
    /// a method is called.
    pub fn new_variant(i2c: I2C, address: u8) -> Self {
        Self {
            i2c,
            address,
            variant: PhantomData,
        }
    }

    /// Gives the bus back.
    pub fn release(self) -> I2C {
        self.i2c
    }

    /// The configured 7-bit address.
    pub fn address(&self) -> u8 {
        self.address
    }

    /// Takes the chip out of software shutdown: writes SSD = 1 to the Shutdown Register (DS
    /// Table 3, p. 8).
    ///
    /// The chip is in software shutdown after power-up and after [`reset`](Self::reset). Leaving
    /// it lights nothing by itself in that state, because the outputs are also switched off in
    /// their LED Control registers. After a [`shutdown`](Self::shutdown) it brings back what was
    /// showing, since the registers kept their data (DS "Shutdown Mode", p. 12).
    pub fn enable(&mut self) -> Result<(), Error<I2C::Error>> {
        self.write(&wire::shutdown(true))
    }

    /// Puts the chip into software shutdown: writes SSD = 0 to the Shutdown Register (DS
    /// Table 3, p. 8).
    ///
    /// All current sources switch off and every register keeps its data (DS "Shutdown Mode" and
    /// "Software Shutdown", p. 12), so this is the way to blank the LEDs without losing the PWM
    /// values and LED Control settings; [`enable`](Self::enable) undoes it.
    pub fn shutdown(&mut self) -> Result<(), Error<I2C::Error>> {
        self.write(&wire::shutdown(false))
    }

    /// Resets every register to its default: writes `00h` to the Reset Register (DS "4Fh Reset
    /// Register", p. 9).
    ///
    /// The defaults are software shutdown, all PWM values 0 and every output off at full current
    /// (DS Table 2, p. 8). Because the chip cannot be read, a reset is the one way to put it into
    /// a known state; bring it up again with [`enable`](Self::enable),
    /// [`set_all_led_controls`](Self::set_all_led_controls) and [`set_frame`](Self::set_frame).
    pub fn reset(&mut self) -> Result<(), Error<I2C::Error>> {
        self.write(&wire::reset())
    }

    /// Stages the PWM duty cycle of one channel (`channel` 0 is OUT1): the average output current
    /// becomes `value / 256` of the output's current (DS Table 4 and Formula (1), p. 8).
    ///
    /// The value goes to a temporary register and shows only after [`update`](Self::update) (DS
    /// "25h PWM Update Register", p. 9). A `channel` of [`Variant::CHANNELS`] or more returns
    /// [`Error::ChannelOutOfRange`] without touching the bus.
    pub fn set_pwm(&mut self, channel: usize, value: u8) -> Result<(), Error<I2C::Error>> {
        self.write(&wire::pwm::<V, _>(channel, value)?)
    }

    /// Writes the PWM values of all channels and latches them: element `n` of `frame` is
    /// channel `n`, OUT(n + 1). A frame is `[u8; 36]`, or `[u8; 28]` on the IS31FL3235 parts.
    ///
    /// This is exactly two I2C writes. The first is 37 bytes (29 on the IS31FL3235 parts), the
    /// address of the first PWM register and then the values, which the chip stores in
    /// consecutive registers by
    /// incrementing the address after each data byte (DS "Address Auto Increment" and Figure 5,
    /// p. 7-8). The second is the update write, which makes all of them take effect in one step (DS
    /// "25h PWM Update Register", p. 9) together with anything else staged before.
    ///
    /// If the first write fails, the update is not sent. The chip may have stored part of the
    /// frame in its temporary registers by then, and the next update write of any kind latches
    /// that part, so after an error send the whole frame again.
    pub fn set_frame(&mut self, frame: &V::Frame) -> Result<(), Error<I2C::Error>> {
        // The address byte and the data have to go out as one write, because every write starts
        // with a register address. `I2c::transaction` with two write operations would save the
        // copy, but only on a bus that joins them without a repeated start as the trait asks,
        // and not every adapter can (MT8163's i2c-dev adapters, for one, do not offer
        // `I2C_FUNC_NOSTART`). Copying 36 bytes works on every bus.
        self.write(wire::frame_block::<V>(frame).as_slice())?;
        // The update is a write of its own and not a 38th byte of the block. The Update Register
        // does follow the last PWM register (`24h`, `25h`), but the datasheet does not say that
        // an auto-incremented write into it latches, and on the IS31FL3235 parts the registers
        // between the last PWM register (`20h`) and `25h` are not documented.
        self.update()
    }

    /// Latches everything staged since the last update: writes `00h` to the Update Register (DS
    /// "25h PWM Update Register", p. 9).
    ///
    /// Needed after [`set_pwm`](Self::set_pwm) and [`set_led_control`](Self::set_led_control).
    /// [`set_frame`](Self::set_frame) and [`set_all_led_controls`](Self::set_all_led_controls)
    /// send it themselves.
    pub fn update(&mut self) -> Result<(), Error<I2C::Error>> {
        self.write(&wire::update())
    }

    /// Stages the LED Control register of one channel (`channel` 0 is OUT1): output on or off,
    /// and its current (DS Table 5, p. 9).
    ///
    /// Like a PWM value, the setting shows only after [`update`](Self::update) (DS "25h PWM
    /// Update Register", p. 9). A `channel` of [`Variant::CHANNELS`] or more returns
    /// [`Error::ChannelOutOfRange`] without touching the bus.
    pub fn set_led_control(
        &mut self,
        channel: usize,
        control: LedControl,
    ) -> Result<(), Error<I2C::Error>> {
        self.write(&wire::led_control::<V, _>(channel, control)?)
    }

    /// Writes the same LED Control setting to all channels and latches it.
    ///
    /// Like [`set_frame`](Self::set_frame) this is two I2C writes: one auto-increment write over
    /// the LED Control registers (DS "Address Auto Increment", p. 7), then the update
    /// write.
    /// The update also latches PWM values staged earlier with [`set_pwm`](Self::set_pwm).
    pub fn set_all_led_controls(&mut self, control: LedControl) -> Result<(), Error<I2C::Error>> {
        self.write(wire::led_controls_block::<V>(control).as_slice())?;
        self.update()
    }

    /// Switches all LEDs off (`false`) or back to normal operation (`true`) through G_EN in the
    /// Global Control Register (DS Table 6, p. 9), keeping every PWM and LED Control setting.
    ///
    /// G_EN = 1 means "shutdown all LEDs", so `false` writes 1. The register is not among the
    /// ones the Update Register text names as latched (`01h~24h`, `26h~49h`; DS p. 9), so it
    /// takes effect when written, without [`update`](Self::update). That is read from the
    /// datasheet; the write has not been tried on a chip. [`shutdown`](Self::shutdown) is the
    /// other way to blank all outputs while keeping the registers.
    pub fn set_global_led_enable(&mut self, enabled: bool) -> Result<(), Error<I2C::Error>> {
        self.write(&wire::global_control(enabled))
    }

    /// One I2C write of `bytes`: the register address and its data (DS Figure 4 "Writing to
    /// IS31FL3236 (Typical)", p. 8, or Figure 5 for an auto-increment block).
    fn write(&mut self, bytes: &[u8]) -> Result<(), Error<I2C::Error>> {
        self.i2c.write(self.address, bytes).map_err(Error::Bus)
    }
}

impl<I2C: I2c, V: SelectableFrequency> Is31fl3236<I2C, V> {
    /// Selects the PWM frequency of all outputs, 3 kHz (the default) or 22 kHz, in the Output
    /// Frequency Setting Register `4Bh` ([`reg::OUTPUT_FREQUENCY`]). Only the "A" variants have
    /// it, so the method exists only for them.
    ///
    /// Like Global Control it is not latched by an update write, so it takes effect when written
    /// (read from the datasheet, not tried on a chip).
    pub fn set_output_frequency(
        &mut self,
        frequency: OutputFrequency,
    ) -> Result<(), Error<I2C::Error>> {
        self.write(&wire::output_frequency(frequency))
    }
}

// One test per write path: they pin the bytes on the bus, which is all a write-only chip has.
#[cfg(test)]
mod tests {
    use super::*;
    use embedded_hal_mock::eh1::i2c::{Mock, Transaction};

    /// The address the tests talk to; any of the four would do.
    const ADDRESS: u8 = reg::ADDRESS_AD_VCC;

    #[test]
    fn set_frame_is_one_pwm_block_write_then_one_update_write() {
        // Distinct values, so that a shifted or reordered frame cannot pass.
        let frame: [u8; CHANNELS] = core::array::from_fn(|channel| 0xa0 + channel as u8);
        let mut block = vec![0x01];
        block.extend_from_slice(&frame);
        assert_eq!(block.len(), 37);
        let expectations = [
            Transaction::write(ADDRESS, block),
            Transaction::write(ADDRESS, vec![0x25, 0x00]),
        ];
        let mut ring = Is31fl3236::new(Mock::new(&expectations), ADDRESS);
        assert_eq!(ring.set_frame(&frame), Ok(()));
        ring.release().done();
    }

    #[test]
    fn set_pwm_stages_one_register_and_update_latches() {
        // Channel 0 is OUT1 at 01h and channel 35 is OUT36 at 24h. Channel 36 would be the
        // Update Register, so it must be refused before anything reaches the bus.
        let expectations = [
            Transaction::write(ADDRESS, vec![0x01, 0x10]),
            Transaction::write(ADDRESS, vec![0x24, 0xff]),
            Transaction::write(ADDRESS, vec![0x25, 0x00]),
        ];
        let mut ring = Is31fl3236::new(Mock::new(&expectations), ADDRESS);
        assert_eq!(ring.set_pwm(0, 0x10), Ok(()));
        assert_eq!(ring.set_pwm(35, 0xff), Ok(()));
        assert_eq!(ring.set_pwm(36, 0xff), Err(Error::ChannelOutOfRange));
        assert_eq!(ring.update(), Ok(()));
        ring.release().done();
    }

    #[test]
    fn enable_and_shutdown_write_the_ssd_bit() {
        let expectations = [
            Transaction::write(ADDRESS, vec![0x00, 0x01]),
            Transaction::write(ADDRESS, vec![0x00, 0x00]),
        ];
        let mut ring = Is31fl3236::new(Mock::new(&expectations), ADDRESS);
        assert_eq!(ring.enable(), Ok(()));
        assert_eq!(ring.shutdown(), Ok(()));
        ring.release().done();
    }

    #[test]
    fn led_control_writes_current_and_on_bit() {
        // 0x07 is on at a quarter of IMAX; 0x02 is off with the current field at a half. Channel 36 would be the Global Control Register.
        let quarter_on = LedControl {
            on: true,
            current: OutputCurrent::Quarter,
        };
        let half_off = LedControl {
            on: false,
            current: OutputCurrent::Half,
        };
        let mut block = vec![0x26];
        block.extend_from_slice(&[0x07; CHANNELS]);
        let expectations = [
            Transaction::write(ADDRESS, vec![0x26, 0x07]),
            Transaction::write(ADDRESS, vec![0x49, 0x02]),
            Transaction::write(ADDRESS, block),
            Transaction::write(ADDRESS, vec![0x25, 0x00]),
        ];
        let mut ring = Is31fl3236::new(Mock::new(&expectations), ADDRESS);
        assert_eq!(ring.set_led_control(0, quarter_on), Ok(()));
        assert_eq!(ring.set_led_control(35, half_off), Ok(()));
        assert_eq!(
            ring.set_led_control(36, quarter_on),
            Err(Error::ChannelOutOfRange)
        );
        assert_eq!(ring.set_all_led_controls(quarter_on), Ok(()));
        ring.release().done();
    }

    #[test]
    fn error_displays_and_is_a_core_error() {
        let bus: Error<embedded_hal::i2c::ErrorKind> =
            Error::Bus(embedded_hal::i2c::ErrorKind::Bus);
        assert_eq!(bus.to_string(), "IS31FL3236 I2C error: Bus");
        let range: Error<embedded_hal::i2c::ErrorKind> = Error::ChannelOutOfRange;
        let dynamic: &dyn core::error::Error = &range;
        assert_eq!(dynamic.to_string(), "IS31FL3236 channel out of range");
    }

    #[test]
    fn global_led_enable_writes_g_en_inverted() {
        // G_EN = 1 is "shutdown all LEDs": disabling writes 1, enabling writes 0. No update.
        let expectations = [
            Transaction::write(ADDRESS, vec![0x4a, 0x01]),
            Transaction::write(ADDRESS, vec![0x4a, 0x00]),
        ];
        let mut ring = Is31fl3236::new(Mock::new(&expectations), ADDRESS);
        assert_eq!(ring.set_global_led_enable(false), Ok(()));
        assert_eq!(ring.set_global_led_enable(true), Ok(()));
        ring.release().done();
    }

    /// The IS31FL3235A: a 28-value frame from 05h, an LED Control block from 2Ah, channel 28
    /// refused, and 4Bh available.
    #[test]
    fn the_3235a_writes_its_own_ranges_and_has_the_frequency_register() {
        let frame: [u8; 28] = core::array::from_fn(|channel| 0xa0 + channel as u8);
        let mut pwm_block = vec![0x05];
        pwm_block.extend_from_slice(&frame);
        let mut control_block = vec![0x2a];
        control_block.extend_from_slice(&[0x07; 28]);
        let expectations = [
            Transaction::write(ADDRESS, pwm_block),
            Transaction::write(ADDRESS, vec![0x25, 0x00]),
            Transaction::write(ADDRESS, control_block),
            Transaction::write(ADDRESS, vec![0x25, 0x00]),
            Transaction::write(ADDRESS, vec![0x20, 0x33]),
            Transaction::write(ADDRESS, vec![0x4b, 0x01]),
        ];
        let mut led =
            Is31fl3236::<_, variant::Is31fl3235A>::new_variant(Mock::new(&expectations), ADDRESS);
        assert_eq!(led.set_frame(&frame), Ok(()));
        assert_eq!(
            led.set_all_led_controls(LedControl {
                on: true,
                current: OutputCurrent::Quarter,
            }),
            Ok(())
        );
        assert_eq!(led.set_pwm(27, 0x33), Ok(()));
        assert_eq!(led.set_pwm(28, 0x33), Err(Error::ChannelOutOfRange));
        assert_eq!(led.set_output_frequency(OutputFrequency::Khz22), Ok(()));
        led.release().done();
    }

    /// The IS31FL3236A: the IS31FL3236's map plus 4Bh.
    #[test]
    fn the_3236a_has_the_3236_map_and_the_frequency_register() {
        let expectations = [
            Transaction::write(ADDRESS, vec![0x24, 0xff]),
            Transaction::write(ADDRESS, vec![0x4b, 0x00]),
        ];
        let mut led =
            Is31fl3236::<_, variant::Is31fl3236A>::new_variant(Mock::new(&expectations), ADDRESS);
        assert_eq!(led.set_pwm(35, 0xff), Ok(()));
        assert_eq!(led.set_output_frequency(OutputFrequency::Khz3), Ok(()));
        led.release().done();
    }

    #[test]
    fn reset_writes_zero_to_the_reset_register() {
        let expectations = [Transaction::write(ADDRESS, vec![0x4f, 0x00])];
        let mut ring = Is31fl3236::new(Mock::new(&expectations), ADDRESS);
        assert_eq!(ring.reset(), Ok(()));
        ring.release().done();
    }
}
