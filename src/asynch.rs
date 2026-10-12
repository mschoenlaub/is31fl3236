//! The same driver over `embedded-hal-async`, for executors such as embassy (feature `async`).
//! Every method sends the byte-for-byte writes of its blocking twin on
//! [`Is31fl3236`](crate::Is31fl3236), built by the crate's private `wire` functions; the
//! method docs there hold the register citations and the order of calls ("Using the driver" in
//! the crate documentation applies unchanged).

use core::marker::PhantomData;

use embedded_hal_async::i2c::I2c;

use crate::variant::{self, SelectableFrequency, Variant};
use crate::{Error, LedControl, OutputFrequency, wire};

/// An IS31FL3236, or a register-compatible variant `V`, on an `embedded-hal-async` I2C bus. See
/// [`crate::Is31fl3236`].
pub struct Is31fl3236Async<I2C, V = variant::Is31fl3236> {
    i2c: I2C,
    address: u8,
    variant: PhantomData<V>,
}

impl<I2C: I2c> Is31fl3236Async<I2C> {
    /// An IS31FL3236 at `address`: see [`Is31fl3236::new`](crate::Is31fl3236::new). Talks to
    /// nothing until a method is awaited.
    pub fn new(i2c: I2C, address: u8) -> Self {
        Self::new_variant(i2c, address)
    }
}

impl<I2C: I2c, V: Variant> Is31fl3236Async<I2C, V> {
    /// The chip `V` at `address`: see [`Is31fl3236::new_variant`](crate::Is31fl3236::new_variant).
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

    /// Leaves software shutdown: see [`Is31fl3236::enable`](crate::Is31fl3236::enable).
    pub async fn enable(&mut self) -> Result<(), Error<I2C::Error>> {
        self.write(&wire::shutdown(true)).await
    }

    /// Enters software shutdown: see [`Is31fl3236::shutdown`](crate::Is31fl3236::shutdown).
    pub async fn shutdown(&mut self) -> Result<(), Error<I2C::Error>> {
        self.write(&wire::shutdown(false)).await
    }

    /// Resets every register: see [`Is31fl3236::reset`](crate::Is31fl3236::reset).
    pub async fn reset(&mut self) -> Result<(), Error<I2C::Error>> {
        self.write(&wire::reset()).await
    }

    /// Stages one channel's PWM value: see [`Is31fl3236::set_pwm`](crate::Is31fl3236::set_pwm).
    pub async fn set_pwm(&mut self, channel: usize, value: u8) -> Result<(), Error<I2C::Error>> {
        self.write(&wire::pwm::<V, _>(channel, value)?).await
    }

    /// Writes and latches all PWM values: see
    /// [`Is31fl3236::set_frame`](crate::Is31fl3236::set_frame).
    pub async fn set_frame(&mut self, frame: &V::Frame) -> Result<(), Error<I2C::Error>> {
        self.write(wire::frame_block::<V>(frame).as_slice()).await?;
        self.update().await
    }

    /// Latches everything staged: see [`Is31fl3236::update`](crate::Is31fl3236::update).
    pub async fn update(&mut self) -> Result<(), Error<I2C::Error>> {
        self.write(&wire::update()).await
    }

    /// Stages one channel's LED Control register: see
    /// [`Is31fl3236::set_led_control`](crate::Is31fl3236::set_led_control).
    pub async fn set_led_control(
        &mut self,
        channel: usize,
        control: LedControl,
    ) -> Result<(), Error<I2C::Error>> {
        self.write(&wire::led_control::<V, _>(channel, control)?)
            .await
    }

    /// Writes and latches the same LED Control setting on all channels: see
    /// [`Is31fl3236::set_all_led_controls`](crate::Is31fl3236::set_all_led_controls).
    pub async fn set_all_led_controls(
        &mut self,
        control: LedControl,
    ) -> Result<(), Error<I2C::Error>> {
        self.write(wire::led_controls_block::<V>(control).as_slice())
            .await?;
        self.update().await
    }

    /// Switches all LEDs off or back on through Global Control: see
    /// [`Is31fl3236::set_global_led_enable`](crate::Is31fl3236::set_global_led_enable).
    pub async fn set_global_led_enable(&mut self, enabled: bool) -> Result<(), Error<I2C::Error>> {
        self.write(&wire::global_control(enabled)).await
    }

    async fn write(&mut self, bytes: &[u8]) -> Result<(), Error<I2C::Error>> {
        self.i2c
            .write(self.address, bytes)
            .await
            .map_err(Error::Bus)
    }
}

impl<I2C: I2c, V: SelectableFrequency> Is31fl3236Async<I2C, V> {
    /// Selects the PWM frequency of all outputs: see
    /// [`Is31fl3236::set_output_frequency`](crate::Is31fl3236::set_output_frequency).
    pub async fn set_output_frequency(
        &mut self,
        frequency: OutputFrequency,
    ) -> Result<(), Error<I2C::Error>> {
        self.write(&wire::output_frequency(frequency)).await
    }
}

// One test per async write path, mirroring the blocking tests: the mock's async impls are the
// blocking ones underneath, so what these pin is that the async driver sends the same bytes in
// the same order, not the bus behaviour.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CHANNELS, OutputCurrent, OutputFrequency, reg, variant};
    use embassy_futures::block_on;
    use embedded_hal_mock::eh1::i2c::{Mock, Transaction};

    const ADDRESS: u8 = reg::ADDRESS_AD_VCC;

    #[test]
    fn set_frame_is_one_pwm_block_write_then_one_update_write() {
        let frame: [u8; CHANNELS] = core::array::from_fn(|channel| 0xa0 + channel as u8);
        let mut block = vec![0x01];
        block.extend_from_slice(&frame);
        let expectations = [
            Transaction::write(ADDRESS, block),
            Transaction::write(ADDRESS, vec![0x25, 0x00]),
        ];
        let mut ring = Is31fl3236Async::new(Mock::new(&expectations), ADDRESS);
        assert_eq!(block_on(ring.set_frame(&frame)), Ok(()));
        ring.release().done();
    }

    #[test]
    fn set_pwm_stages_one_register_and_update_latches() {
        let expectations = [
            Transaction::write(ADDRESS, vec![0x01, 0x10]),
            Transaction::write(ADDRESS, vec![0x24, 0xff]),
            Transaction::write(ADDRESS, vec![0x25, 0x00]),
        ];
        let mut ring = Is31fl3236Async::new(Mock::new(&expectations), ADDRESS);
        assert_eq!(block_on(ring.set_pwm(0, 0x10)), Ok(()));
        assert_eq!(block_on(ring.set_pwm(35, 0xff)), Ok(()));
        assert_eq!(
            block_on(ring.set_pwm(36, 0xff)),
            Err(Error::ChannelOutOfRange)
        );
        assert_eq!(block_on(ring.update()), Ok(()));
        ring.release().done();
    }

    #[test]
    fn enable_shutdown_and_reset_write_their_registers() {
        let expectations = [
            Transaction::write(ADDRESS, vec![0x00, 0x01]),
            Transaction::write(ADDRESS, vec![0x00, 0x00]),
            Transaction::write(ADDRESS, vec![0x4f, 0x00]),
        ];
        let mut ring = Is31fl3236Async::new(Mock::new(&expectations), ADDRESS);
        assert_eq!(block_on(ring.enable()), Ok(()));
        assert_eq!(block_on(ring.shutdown()), Ok(()));
        assert_eq!(block_on(ring.reset()), Ok(()));
        ring.release().done();
    }

    #[test]
    fn global_enable_frequency_and_a_3235a_frame() {
        let frame: [u8; 28] = core::array::from_fn(|channel| channel as u8);
        let mut block = vec![0x05];
        block.extend_from_slice(&frame);
        let expectations = [
            Transaction::write(ADDRESS, vec![0x4a, 0x01]),
            Transaction::write(ADDRESS, block),
            Transaction::write(ADDRESS, vec![0x25, 0x00]),
            Transaction::write(ADDRESS, vec![0x4b, 0x01]),
        ];
        let mut led = Is31fl3236Async::<_, variant::Is31fl3235A>::new_variant(
            Mock::new(&expectations),
            ADDRESS,
        );
        assert_eq!(block_on(led.set_global_led_enable(false)), Ok(()));
        assert_eq!(block_on(led.set_frame(&frame)), Ok(()));
        assert_eq!(
            block_on(led.set_output_frequency(OutputFrequency::Khz22)),
            Ok(())
        );
        led.release().done();
    }

    #[test]
    fn led_control_writes_current_and_on_bit() {
        let quarter_on = LedControl {
            on: true,
            current: OutputCurrent::Quarter,
        };
        let mut block = vec![0x26];
        block.extend_from_slice(&[0x07; CHANNELS]);
        let expectations = [
            Transaction::write(ADDRESS, vec![0x26, 0x07]),
            Transaction::write(ADDRESS, block),
            Transaction::write(ADDRESS, vec![0x25, 0x00]),
        ];
        let mut ring = Is31fl3236Async::new(Mock::new(&expectations), ADDRESS);
        assert_eq!(block_on(ring.set_led_control(0, quarter_on)), Ok(()));
        assert_eq!(
            block_on(ring.set_led_control(36, quarter_on)),
            Err(Error::ChannelOutOfRange)
        );
        assert_eq!(block_on(ring.set_all_led_controls(quarter_on)), Ok(()));
        ring.release().done();
    }
}
