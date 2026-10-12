# is31fl3236

[![crates.io](https://img.shields.io/crates/v/is31fl3236.svg)](https://crates.io/crates/is31fl3236)
[![docs.rs](https://docs.rs/is31fl3236/badge.svg)](https://docs.rs/is31fl3236)
[![CI](https://github.com/mschoenlaub/is31fl3236/actions/workflows/ci.yml/badge.svg)](https://github.com/mschoenlaub/is31fl3236/actions/workflows/ci.yml)

`no_std` driver for the Lumissil **IS31FL3236** and its register-compatible relatives, the
IS31FL3236A, IS32FL3236A, IS31FL3235 and IS31FL3235A: constant-current LED drivers with 36 or 28
channels, 8-bit PWM per channel and an I2C interface. It depends only on `embedded-hal` 1.0 (and
`embedded-hal-async` with the feature `async`), allocates nothing and keeps no copy of the chip's
state.

## Usage

```rust
use embedded_hal::i2c::I2c;
use is31fl3236::{CHANNELS, Error, Is31fl3236, LedControl, OutputCurrent, reg};

fn light_up<I2C: I2c>(i2c: I2C) -> Result<I2C, Error<I2C::Error>> {
    // The address follows the chip's AD pin: GND, SCL, SDA or VCC (0x3c..=0x3f).
    let mut leds = Is31fl3236::new(i2c, reg::ADDRESS_AD_VCC);

    // Start from a known state: the chip cannot be read, so a reset is the only way to know.
    // It leaves the chip in software shutdown with every output off.
    leds.reset()?;
    leds.enable()?;

    // Switch all outputs on, each at a quarter of the maximum current.
    leds.set_all_led_controls(LedControl {
        on: true,
        current: OutputCurrent::Quarter,
    })?;

    // One frame: a PWM value per output, element 0 for OUT1. All outputs change in one step.
    let mut frame = [0u8; CHANNELS];
    frame[0] = 0x80;
    leds.set_frame(&frame)?;

    // Single channels are staged and show together on update().
    leds.set_pwm(1, 0x40)?;
    leds.set_pwm(2, 0x20)?;
    leds.update()?;

    // Dark without losing the registers; enable() brings the picture back.
    leds.shutdown()?;

    Ok(leds.release())
}
```

Another chip of the family is a type parameter. Its frame has its own length, and the "A" parts
have the PWM frequency setting:

```rust
use embedded_hal::i2c::I2c;
use is31fl3236::{Error, Is31fl3236, OutputFrequency, reg, variant};

fn quiet_28<I2C: I2c>(i2c: I2C) -> Result<(), Error<I2C::Error>> {
    let mut leds = Is31fl3236::<_, variant::Is31fl3235A>::new_variant(i2c, reg::ADDRESS_AD_GND);
    leds.set_output_frequency(OutputFrequency::Khz22)?;
    leds.set_frame(&[0x10; 28])
}
```

## Supported chips

| Type (`variant::`) | Chips | Outputs | PWM | LED Control | Output frequency `4Bh` |
| --- | --- | --- | --- | --- | --- |
| `Is31fl3236` (default) | IS31FL3236 | 36 | `01h–24h` | `26h–49h` | no |
| `Is31fl3236A` | IS31FL3236A, IS32FL3236A | 36 | `01h–24h` | `26h–49h` | yes |
| `Is31fl3235` | IS31FL3235 | 28 | `05h–20h` | `2Ah–45h` | no |
| `Is31fl3235A` | IS31FL3235A | 28 | `05h–20h` | `2Ah–45h` | yes |

All of them share the slave address (`01111` then the AD pin), Shutdown `00h`, Update `25h`,
Global Control `4Ah`, Reset `4Fh` and the LED Control bits. `set_output_frequency` exists only for
the "A" types; on the others it does not compile.

## Things to know

- **The chip is write-only.** The driver cannot find out what the chip holds, so start from
  `reset()`. There are no read methods.
- **Channels count from zero**: channel `n`, and element `n` of a frame, is OUT(n + 1). A channel
  beyond the chip's outputs returns `Error::ChannelOutOfRange` and sends nothing.
- **PWM and LED Control writes are staged** in the chip until an update write. `set_frame` and
  `set_all_led_controls` send the update themselves; `set_pwm` and `set_led_control` do not, so
  call `update()` after them. An update latches everything staged so far, of both kinds.
- **Three things before an LED lights** after a reset or power-up: `enable()`, the output switched
  on in its LED Control register, and a PWM value.
- **`OutputCurrent` divides, it does not dim linearly**: `Full`, `Half`, `Third` and `Quarter` are
  `IMAX`, `IMAX/2`, `IMAX/3` and `IMAX/4`, where `IMAX` is set by the resistor on R_EXT.
- **A frame is two I2C writes**: the PWM block (37 bytes, 29 on the IS31FL3235 parts), then the
  update. Until the update the outputs keep the old frame, so nothing tears. If the block fails, or
  an async `set_frame` is dropped between the two writes, the staged values show with the next
  update of any kind: send the whole frame again.
- **`set_global_led_enable` and `set_output_frequency` act at once**, without an update: the
  datasheets name only the PWM and LED Control ranges as latched.
- **Hardware shutdown** is the chip's SDB pin, a GPIO of the board, not part of this driver.

## Features

- `async`: `asynch::Is31fl3236Async`, the same driver over `embedded-hal-async`, sending the same
  bytes.

## Status

The IS31FL3236 has driven the LED ring of a 2nd-generation Echo Dot from bare metal since
2026-10-02, with both the blocking and the async driver. The other variants are implemented from
their datasheets and pinned by mock tests; they have not run on a chip yet.

## Testing

`cargo test` runs one `embedded-hal-mock` test per write path for the blocking and the async
driver, tests of the byte encoding for every variant, and this README's examples as doctests,
including one that checks `set_output_frequency` does not compile on the plain IS31FL3236.

## Sources

- Lumissil, [IS31FL3236](https://www.lumissil.com/assets/pdf/core/IS31FL3236_DS.pdf), Rev. H, 2021
  (cited as "DS"): I2C interface and slave address (Table 1), address auto-increment (Figures 4
  and 5), register map (Table 2), register descriptions (Tables 3 to 6), `IMAX` (Formula 2),
  shutdown modes.
- Lumissil, [IS31FL3236A](https://www.lumissil.com/assets/pdf/core/IS31FL3236A_DS.pdf), Rev. I,
  2024: the same map plus `4Bh` (Tables 2 and 7). The IS32FL3236A is its automotive version.
- Lumissil, [IS31FL3235](https://www.lumissil.com/assets/pdf/core/IS31FL3235_DS.pdf), Rev. E, 2021,
  and [IS31FL3235A](https://www.lumissil.com/assets/pdf/core/IS31FL3235A_DS.pdf), Rev. J, 2024: the
  28-channel map (Table 2).

## Minimum supported Rust version

Rust 1.85 (edition 2024), tested in CI. Raising it is a minor version bump.

## License

Licensed under either of [Apache License, Version 2.0](https://github.com/mschoenlaub/is31fl3236/blob/main/LICENSE-APACHE)
or [MIT license](https://github.com/mschoenlaub/is31fl3236/blob/main/LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
