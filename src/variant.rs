//! The chips this driver covers: the IS31FL3236 and its register-compatible relatives.
//!
//! All of them have the same slave address (`01111`, then the AD pin), the same registers for
//! shutdown (`00h`), update (`25h`), global control (`4Ah`) and reset (`4Fh`), the same LED
//! Control bits (SL current and OUT) and 8-bit PWM. They differ in three things, which a
//! [`Variant`] states:
//!
//! - the number of outputs, 36 or 28;
//! - where the per-output ranges start: the IS31FL3235 parts keep the IS31FL3236's address map
//!   but have no OUT1 to OUT4 registers there, so their OUT1 is at `05h` (PWM) and `2Ah` (LED
//!   Control);
//! - whether the Output Frequency Setting Register `4Bh` exists: only on the "A" parts
//!   ([`SelectableFrequency`]).
//!
//! | Marker | Chips | Outputs | PWM | LED Control | `4Bh` | Datasheet |
//! | --- | --- | --- | --- | --- | --- | --- |
//! | [`Is31fl3236`] | IS31FL3236 | 36 | `01h–24h` | `26h–49h` | no | Rev. H, 2021 |
//! | [`Is31fl3236A`] | IS31FL3236A, IS32FL3236A | 36 | `01h–24h` | `26h–49h` | yes | Rev. I, 2024 |
//! | [`Is31fl3235`] | IS31FL3235 | 28 | `05h–20h` | `2Ah–45h` | no | Rev. E, 2021 |
//! | [`Is31fl3235A`] | IS31FL3235A | 28 | `05h–20h` | `2Ah–45h` | yes | Rev. J, 2024 |
//!
//! Each datasheet's Table 2 (register map) and Table 1 (slave address) are the sources; the
//! IS32FL3236A is the automotive (AEC-Q100) version of the IS31FL3236A with the same register
//! map. Only the plain IS31FL3236 has run on a chip with this driver (the 2nd-generation Echo
//! Dot's LED ring); the others are implemented from their datasheets and pinned by mock tests.

mod sealed {
    pub trait Sealed {}
}

/// A chip of the IS31FL3236 family: what differs between them. Implemented by the markers of
/// this module only.
pub trait Variant: sealed::Sealed {
    /// The chip's name, for messages.
    const NAME: &'static str;
    /// Its outputs, OUT1 to OUT`CHANNELS`.
    const CHANNELS: usize;
    /// The PWM register of OUT1; the others follow, one per output.
    const PWM_FIRST: u8;
    /// The LED Control register of OUT1; the others follow, one per output.
    const LED_CONTROL_FIRST: u8;
    /// A frame: one PWM value per output, element `n` for OUT(n + 1).
    type Frame: AsRef<[u8]>;
}

/// The variants with the Output Frequency Setting Register `4Bh`, which selects 3 kHz or 22 kHz
/// PWM ([`Is31fl3236::set_output_frequency`](crate::Is31fl3236::set_output_frequency)).
///
/// On an "A" part the method is there:
///
/// ```
/// use embedded_hal::i2c::I2c;
/// use is31fl3236::{Error, Is31fl3236, OutputFrequency, variant};
///
/// fn quiet<I: I2c>(led: &mut Is31fl3236<I, variant::Is31fl3236A>) -> Result<(), Error<I::Error>> {
///     led.set_output_frequency(OutputFrequency::Khz22)
/// }
/// ```
///
/// On the plain IS31FL3236, which has no `4Bh`, it does not compile:
///
/// ```compile_fail
/// use embedded_hal::i2c::I2c;
/// use is31fl3236::{Error, Is31fl3236, OutputFrequency};
///
/// fn quiet<I: I2c>(led: &mut Is31fl3236<I>) -> Result<(), Error<I::Error>> {
///     led.set_output_frequency(OutputFrequency::Khz22)
/// }
/// ```
pub trait SelectableFrequency: Variant {}

/// The IS31FL3236: 36 outputs, no `4Bh` (DS "IS31FL3236", Rev. H, Table 2). The driver's
/// default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Is31fl3236;

/// The IS31FL3236A, and the IS32FL3236A with the same register map: the IS31FL3236 plus `4Bh`
/// (DS "IS31FL3236A", Rev. I, Table 2 and Table 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Is31fl3236A;

/// The IS31FL3235: 28 outputs, PWM `05h–20h`, LED Control `2Ah–45h`, no `4Bh` (DS "IS31FL3235",
/// Rev. E, Table 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Is31fl3235;

/// The IS31FL3235A: the IS31FL3235 plus `4Bh` (DS "IS31FL3235A", Rev. J, Table 2 and Table 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Is31fl3235A;

impl sealed::Sealed for Is31fl3236 {}
impl sealed::Sealed for Is31fl3236A {}
impl sealed::Sealed for Is31fl3235 {}
impl sealed::Sealed for Is31fl3235A {}

impl Variant for Is31fl3236 {
    const NAME: &'static str = "IS31FL3236";
    const CHANNELS: usize = 36;
    const PWM_FIRST: u8 = 0x01;
    const LED_CONTROL_FIRST: u8 = 0x26;
    type Frame = [u8; 36];
}

impl Variant for Is31fl3236A {
    const NAME: &'static str = "IS31FL3236A";
    const CHANNELS: usize = 36;
    const PWM_FIRST: u8 = 0x01;
    const LED_CONTROL_FIRST: u8 = 0x26;
    type Frame = [u8; 36];
}

impl Variant for Is31fl3235 {
    const NAME: &'static str = "IS31FL3235";
    const CHANNELS: usize = 28;
    const PWM_FIRST: u8 = 0x05;
    const LED_CONTROL_FIRST: u8 = 0x2a;
    type Frame = [u8; 28];
}

impl Variant for Is31fl3235A {
    const NAME: &'static str = "IS31FL3235A";
    const CHANNELS: usize = 28;
    const PWM_FIRST: u8 = 0x05;
    const LED_CONTROL_FIRST: u8 = 0x2a;
    type Frame = [u8; 28];
}

impl SelectableFrequency for Is31fl3236A {}
impl SelectableFrequency for Is31fl3235A {}

// Every range must fit below the next register of the shared map: the PWM range ends before the
// Update Register (`25h`) and the LED Control range before the Global Control Register (`4Ah`).
// A wrong base or count would otherwise let a block write run into those registers.
// (A macro rather than a `const fn`: Rust 1.85 reports a function used only in `const _` items as
// dead code.)
macro_rules! assert_fits {
    ($($v:ty),*) => {$(
        const _: () = assert!(
            <$v>::PWM_FIRST as usize + <$v>::CHANNELS <= crate::reg::UPDATE as usize
                && <$v>::LED_CONTROL_FIRST as usize + <$v>::CHANNELS
                    <= crate::reg::GLOBAL_CONTROL as usize
        );
    )*};
}
assert_fits!(Is31fl3236, Is31fl3236A, Is31fl3235, Is31fl3235A);
// The 3235 parts' ranges end exactly where their datasheet says: `20h` and `45h`.
const _: () = assert!(Is31fl3235::PWM_FIRST as usize + Is31fl3235::CHANNELS - 1 == 0x20);
const _: () = assert!(Is31fl3235::LED_CONTROL_FIRST as usize + Is31fl3235::CHANNELS - 1 == 0x45);
