//! Register map of the IS31FL3236, read from the manufacturer's datasheet.
//!
//! Every citation below is to "IS31FL3236, 36 Channels LED Driver", Lumissil Microsystems,
//! Rev. H, 10/21/2021 (`https://www.lumissil.com/assets/pdf/core/IS31FL3236_DS.pdf`), written as
//! "DS" plus the table or section and the page. The IS31FL3236A is a different part with its own
//! datasheet and one more register; see the crate documentation.
//!
//! All registers are write-only (DS "I2C Interface" and Table 1, p. 7), so there is no read side
//! to this map.

use crate::CHANNELS;

/// 7-bit bus address with the AD pin tied to GND.
///
/// DS Table 1 "Slave Address (Write Only)", p. 7: the address byte is `01111`, then two bits set
/// by the AD pin, then the R/W bit, which is always 0. The 7-bit address is therefore
/// `0b01111_00 | AD`, and AD = `00` when the pin is connected to GND.
pub const ADDRESS_AD_GND: u8 = 0x3c;

/// 7-bit bus address with the AD pin tied to SCL: AD = `01` (DS Table 1, p. 7).
pub const ADDRESS_AD_SCL: u8 = 0x3d;

/// 7-bit bus address with the AD pin tied to SDA: AD = `10` (DS Table 1, p. 7).
pub const ADDRESS_AD_SDA: u8 = 0x3e;

/// 7-bit bus address with the AD pin tied to VCC: AD = `11` (DS Table 1, p. 7).
pub const ADDRESS_AD_VCC: u8 = 0x3f;

/// Shutdown Register, `00h` (DS Table 2, p. 8, and Table 3, p. 8). Default `0000 0000`, which
/// means the chip comes out of power-up and out of a reset in software shutdown.
pub const SHUTDOWN: u8 = 0x00;

/// SSD, bit D0 of [`SHUTDOWN`] (DS Table 3, p. 8): 1 is normal operation, 0 is software shutdown.
///
/// The polarity is the opposite of the register's name: writing the bit takes the chip *out* of
/// shutdown. In software shutdown all current sources are off and the registers keep their data
/// (DS "Shutdown Mode" and "Software Shutdown", p. 12). Bits D7:D1 are unused and default to 0.
pub const SHUTDOWN_SSD: u8 = 0b0000_0001;

/// PWM Register of OUT1, `01h`: the first of 36 consecutive registers, one per output in output
/// order (DS Table 2, p. 8, "01h~24h"; Table 4 "01h~24h PWM Register (OUT1~OUT36)", p. 8).
///
/// Each holds an 8-bit duty cycle: the average output current is `IOUT * value / 256` (DS
/// Formula (1), p. 8), so 255 is not quite continuous. A written value sits in a temporary
/// register and takes effect only on the next [`UPDATE`] write (DS "25h PWM Update Register",
/// p. 9).
pub const PWM_FIRST: u8 = 0x01;

/// PWM Register of OUT36, `24h`: the last of the range that starts at [`PWM_FIRST`] (DS Table 2,
/// p. 8).
pub const PWM_LAST: u8 = 0x24;

/// Update Register, `25h` (DS Table 2, p. 8; "25h PWM Update Register", p. 9).
///
/// Writing [`UPDATE_LATCH`] here copies the temporary copies of the PWM registers
/// (`01h~24h`) and of the LED Control registers (`26h~49h`) into effect at once. The latch is what
/// makes a 36-byte frame change in one step instead of channel by channel.
pub const UPDATE: u8 = 0x25;

/// The value the datasheet requires for an [`UPDATE`] write: "a write operation of `0000 0000`
/// value to the Update Register is required to update the registers" (DS p. 9).
pub const UPDATE_LATCH: u8 = 0x00;

/// LED Control Register of OUT1, `26h`: the first of 36 consecutive registers, one per output in
/// output order (DS Table 2, p. 8, "26h~49h"; Table 5 "26h~49h LED Control Register
/// (OUT1~OUT36)", p. 9).
///
/// Each holds the output's on/off bit ([`LED_CONTROL_OUT`]) and its current setting
/// ([`LED_CONTROL_SL_MASK`]). Like the PWM registers, a written value takes effect only on the
/// next [`UPDATE`] write (DS "25h PWM Update Register", p. 9). Default `0000 0000`: every output
/// off.
pub const LED_CONTROL_FIRST: u8 = 0x26;

/// LED Control Register of OUT36, `49h`: the last of the range that starts at
/// [`LED_CONTROL_FIRST`] (DS Table 2, p. 8).
pub const LED_CONTROL_LAST: u8 = 0x49;

/// OUT, bit D0 of an LED Control register (DS Table 5, p. 9): 1 is LED on, 0 is LED off.
pub const LED_CONTROL_OUT: u8 = 0b0000_0001;

/// SL, bits D2:D1 of an LED Control register (DS Table 5, p. 9): the output current setting,
/// one of the [`OutputCurrent`] values. Bits D7:D3 are unused and default to 0.
pub const LED_CONTROL_SL_MASK: u8 = 0b0000_0110;

/// How far an [`OutputCurrent`] value is shifted left to land in SL, bits D2:D1 (DS Table 5,
/// p. 9).
pub const LED_CONTROL_SL_SHIFT: u8 = 1;

/// The values of the SL field of an LED Control register (DS Table 5, p. 9, "Output Current
/// Setting (IOUT)"), unshifted.
///
/// `IMAX` is the chip-wide maximum set by the external resistor on R_EXT:
/// `IMAX = 58.5 * 1.3 V / REXT` (DS Formula (2), p. 11), which is 23 mA at 3.3 kOhm and 38 mA
/// at the recommended minimum of 2 kOhm (DS "Electrical Characteristics", p. 5). The field
/// scales one output against that maximum; it is not a brightness level, and the larger the
/// value the dimmer the LED.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OutputCurrent {
    /// SL = `00`: `IMAX`.
    Full = 0b00,
    /// SL = `01`: `IMAX / 2`.
    Half = 0b01,
    /// SL = `10`: `IMAX / 3`.
    Third = 0b10,
    /// SL = `11`: `IMAX / 4`.
    Quarter = 0b11,
}

/// Global Control Register, `4Ah` (DS Table 2, p. 8, and Table 6, p. 9). Default `0000 0000`.
pub const GLOBAL_CONTROL: u8 = 0x4a;

/// G_EN, bit D0 of [`GLOBAL_CONTROL`] (DS Table 6, p. 9): 0 is normal operation, 1 shuts down
/// all LEDs.
///
/// Despite being called "Global LED Enable", setting the bit turns every output *off*. The
/// Update Register text on p. 9 names only `01h~24h` and `26h~49h` as latched, so this register
/// presumably acts as soon as it is written; the datasheet does not say so outright, and nothing
/// here has been tried on a chip. Bits D7:D1 are unused and default to 0.
pub const GLOBAL_CONTROL_G_EN: u8 = 0b0000_0001;

/// Output Frequency Setting Register, `4Bh`, on the "A" parts only: IS31FL3236A (DS
/// "IS31FL3236A", Rev. I, Table 2, p. 8, and Table 7, p. 9), IS32FL3236A and IS31FL3235A.
/// Default `0000 0000`: 3 kHz. The IS31FL3236 and IS31FL3235 do not have it, so the driver
/// offers it only for the variants that do ([`crate::variant::SelectableFrequency`]).
///
/// Like [`GLOBAL_CONTROL`], it is not among the registers the Update Register text names as
/// latched (`01h~24h` and `26h~49h`), so it presumably acts as soon as it is written.
pub const OUTPUT_FREQUENCY: u8 = 0x4b;

/// The values of OFS, bit D0 of [`OUTPUT_FREQUENCY`] (IS31FL3236A DS Table 7, p. 9): the PWM
/// frequency of all outputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum OutputFrequency {
    /// OFS = 0: 3 kHz, the default.
    #[default]
    Khz3 = 0,
    /// OFS = 1: 22 kHz.
    Khz22 = 1,
}

/// Reset Register, `4Fh` (DS Table 2, p. 8; "4Fh Reset Register", p. 9).
///
/// Writing [`RESET_VALUE`] puts every register back to its default. Those defaults are a blank
/// display *and* software shutdown ([`SHUTDOWN`] defaults to 0), so after a reset nothing lights
/// until [`SHUTDOWN_SSD`] is written again.
pub const RESET: u8 = 0x4f;

/// The value the datasheet requires for a [`RESET`] write: "once user writes `0000 0000` data to
/// the Reset Register, IS31FL3236 will reset all registers to default value" (DS p. 9).
pub const RESET_VALUE: u8 = 0x00;

// The two per-output ranges must each hold exactly one register per channel, or a frame written
// with address auto-increment would run into the neighbouring register. Checking it at compile
// time costs nothing and catches a mistyped bound.
const _: () = assert!((PWM_LAST - PWM_FIRST) as usize + 1 == CHANNELS);
const _: () = assert!((LED_CONTROL_LAST - LED_CONTROL_FIRST) as usize + 1 == CHANNELS);
