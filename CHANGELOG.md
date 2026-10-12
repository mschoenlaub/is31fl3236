# Changelog

All notable changes to this crate are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crate follows
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-10-12

First release.

- The IS31FL3236 and its register-compatible relatives as type parameters (`variant`): the
  IS31FL3236A and IS32FL3236A (36 outputs, with `4Bh`), the IS31FL3235 and IS31FL3235A (28 outputs,
  PWM from `05h`, LED Control from `2Ah`).
- Software shutdown, reset, PWM per channel and per frame (one auto-increment block and the update
  write), the update latch, LED Control (on/off and output current) per channel and for all
  channels.
- Global Control (`set_global_led_enable`) on all variants, and the PWM frequency
  (`set_output_frequency`, 3 kHz or 22 kHz) on the "A" parts only.
- The same driver over `embedded-hal-async`, behind the feature `async`.

[Unreleased]: https://github.com/mschoenlaub/is31fl3236/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/mschoenlaub/is31fl3236/releases/tag/v0.1.0
