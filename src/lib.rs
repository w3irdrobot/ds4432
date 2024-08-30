//! DS4432 driver.
//!
//! The DS4432 contains two I2C programmable current
//! DACs that are each capable of sinking and sourcing
//! current up to 200µA. Each DAC output has 127 sink
//! and 127 source settings that are programmed using the
//! I2C interface. The current DAC outputs power up in a
//! high-impedance state.
//!
//! - [DS4432 product page](https://www.digikey.com/en/products/detail/analog-devices-inc-maxim-integrated/DS4432U-T-R/2062898)
//! - [DS4432 datasheet](https://www.analog.com/media/en/technical-documentation/data-sheets/DS4432.pdf)

#![no_std]
#![macro_use]
pub(crate) mod fmt;

mod error;
pub use error::{Error, Result};

#[cfg(any(feature = "async", feature = "sync"))]
use embedded_hal::i2c::ErrorType;
#[cfg(feature = "sync")]
use embedded_hal::i2c::I2c;
#[cfg(feature = "async")]
use embedded_hal_async::i2c::I2c as AsyncI2c;

/// The DS4432's I2C addresses.
#[cfg(any(feature = "async", feature = "sync"))]
const SLAVE_ADDRESS: u8 = 0x90;

/// An output controllable by the DS4432. This device has two.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
#[repr(u8)]
pub enum Output {
    One = 0xF8,
    Two = 0xF9,
}

impl From<Output> for u8 {
    fn from(value: Output) -> Self {
        value as u8
    }
}

/// The status of an output.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum Status {
    /// The output should sink at the given code
    Sink(u8),
    /// The output should source at the given code
    Source(u8),
    /// The output is completely disabled
    Disable,
}

impl Status {
    pub fn code(&self) -> u8 {
        match self {
            Self::Sink(c) | Self::Source(c) => *c,
            Self::Disable => 0,
        }
    }
}

impl From<u8> for Status {
    fn from(value: u8) -> Self {
        let sourcing = value & 0x80 == 0x80;
        let code = value & 0x7F;

        match (sourcing, code) {
            (true, 0) => Self::Disable,
            (false, 0) => Self::Disable,
            (true, c) => Self::Source(c),
            (false, c) => Self::Sink(c),
        }
    }
}

/// A DS4432 Digital To Analog (DAC) converter on the I2C bus `I`.
#[maybe_async_cfg::maybe(
    sync(feature = "sync", self = "DS4432"),
    async(feature = "async", keep_self)
)]
pub struct AsyncDS4432<I> {
    i2c: I,
}

#[maybe_async_cfg::maybe(
    sync(feature = "sync", self = "DS4432", idents(AsyncI2c(sync = "I2c"))),
    async(feature = "async", keep_self)
)]
impl<I: AsyncI2c + ErrorType> AsyncDS4432<I> {
    /// Create a new DS4432 using the given I2C implementation
    pub async fn new(i2c: I) -> Self {
        trace!("new");
        Self { i2c }
    }

    /// Set the current sink/source status and code of an output
    pub async fn set_status(&mut self, output: Output, status: Status) -> Result<(), I::Error> {
        trace!("set_status");

        let reg = output.into();
        let value = match status {
            Status::Disable | Status::Sink(0) | Status::Source(0) => 0,
            Status::Sink(code) => {
                if code > 127 {
                    return Err(Error::InvalidCode(code));
                } else {
                    code
                }
            }
            Status::Source(code) => {
                if code > 127 {
                    return Err(Error::InvalidCode(code));
                } else {
                    // ensures MSB is 1
                    code | 0x80
                }
            }
        };

        debug!("W @0x{:x}={:x}", reg, value);

        self.i2c
            .write(SLAVE_ADDRESS, &[reg, value])
            .await
            .map_err(Error::I2c)
    }

    /// Get the current sink/source status and code of an output
    pub async fn status(&mut self, output: Output) -> Result<Status, I::Error> {
        trace!("status");

        let mut buf = [0x00];
        let reg = output.into();

        self.i2c
            .write_read(SLAVE_ADDRESS, &[reg], &mut buf)
            .await
            .map_err(Error::I2c)?;

        debug!("R @0x{:x}={:x}", reg, buf[0]);

        Ok(buf[0].into())
    }

    /// Return the underlying I2C device
    pub fn release(self) -> I {
        self.i2c
    }

    /// Destroys this driver and releases the I2C bus `I`.
    pub fn destroy(self) -> Self {
        self
    }
}

#[cfg(test)]
mod test {
    // extern crate alloc;
    extern crate std;

    use super::*;
    use embedded_hal_mock::eh1::i2c;
    use std::vec;

    #[test]
    fn can_get_output_1_status() {
        let expectations = [i2c::Transaction::write_read(
            SLAVE_ADDRESS,
            vec![Output::One as u8],
            vec![0xAA],
        )];
        let mock = i2c::Mock::new(&expectations);
        let mut ds4432 = DS4432::new(mock);

        let status = ds4432.status(Output::One).unwrap();
        assert!(matches!(status, Status::Source(42)));

        let mut mock = ds4432.release();
        mock.done();
    }

    #[test]
    fn can_set_output_2_status() {
        let expectations = [i2c::Transaction::write(
            SLAVE_ADDRESS,
            vec![Output::Two as u8, 0x2A],
        )];
        let mock = i2c::Mock::new(&expectations);
        let mut ds4432 = DS4432::new(mock);

        // just making sure it doesn't error
        ds4432.set_status(Output::Two, Status::Sink(42)).unwrap();

        let mut mock = ds4432.release();
        mock.done();
    }

    #[test]
    fn status_to_code_conversion() {
        assert_eq!(Status::Sink(42).code(), 0x2A);
        assert_eq!(Status::Source(42).code(), 0x2A);
        assert_eq!(Status::Disable.code(), 0x00);
        assert_eq!(Status::Sink(0).code(), 0x00);
        assert_eq!(Status::Source(0).code(), 0x00);
    }

    #[test]
    fn u8_to_status_conversion() {
        assert_eq!(Status::from(0x2A), Status::Sink(42));
        assert_eq!(Status::from(0xAA), Status::Source(42));
        assert_eq!(Status::from(0x00), Status::Disable);
        assert_eq!(Status::from(0x80), Status::Disable);
    }
}
