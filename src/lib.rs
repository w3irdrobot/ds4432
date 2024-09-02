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

#[cfg(not(any(feature = "sync", feature = "async")))]
compile_error!("You should probably choose at least one of `sync` and `async` features.");

#[cfg(feature = "sync")]
use embedded_hal::i2c::ErrorType;
#[cfg(feature = "sync")]
use embedded_hal::i2c::I2c;
#[cfg(feature = "async")]
use embedded_hal_async::i2c::ErrorType as AsyncErrorType;
#[cfg(feature = "async")]
use embedded_hal_async::i2c::I2c as AsyncI2c;

/// The DS4432's I2C addresses.
#[cfg(any(feature = "async", feature = "sync"))]
const SLAVE_ADDRESS: u8 = 0b1001000; // This is I2C address 0x48

#[cfg(not(feature = "not-recommended-rfs"))]
const RECOMMENDED_RFS_MIN: u32 = 40_000;
#[cfg(not(feature = "not-recommended-rfs"))]
const RECOMMENDED_RFS_MAX: u32 = 160_000;

/// An output controllable by the DS4432. This device has two.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
#[repr(u8)]
pub enum Output {
    Zero = 0xF8,
    One = 0xF9,
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
    /// The output is completely disabled
    Disable,
    /// The output sink at the given code
    Sink(u8),
    /// The output sink at the given current value
    SinkMicroAmp(f32),
    /// The output source at the given code
    Source(u8),
    /// The output source at the given current value
    SourceMicroAmp(f32),
}

impl Status {
    pub fn code(&self) -> Option<u8> {
        match self {
            Self::Sink(c) | Self::Source(c) => Some(*c),
            Self::Disable => Some(0),
            _ => None,
        }
    }

    pub fn current_ua(&self, rfs_ohm: u32) -> Option<f32> {
        self.code()
            .map(|code| ((62_312.5 * code as f64) / (rfs_ohm as f64)) as f32)
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
    rfs0_ohm: Option<u32>,
    rfs1_ohm: Option<u32>,
}

#[maybe_async_cfg::maybe(
    sync(
        feature = "sync",
        self = "DS4432",
        idents(AsyncI2c(sync = "I2c"), AsyncErrorType(sync = "ErrorType"))
    ),
    async(feature = "async", keep_self)
)]
impl<I: AsyncI2c + AsyncErrorType> AsyncDS4432<I> {
    /// Create a new DS4432 using the given I2C implementation
    pub fn new(i2c: I) -> Self {
        trace!("new");
        Self::with_rfs(i2c, None, None).unwrap()
    }

    /// Create a new DS4432 using the given I2C implementation and the optinal Rfs values
    pub fn with_rfs(
        i2c: I,
        rfs0_ohm: Option<u32>,
        rfs1_ohm: Option<u32>,
    ) -> Result<Self, I::Error> {
        for rfs_ohm in [rfs0_ohm, rfs1_ohm] {
            if let Some(rfs) = rfs_ohm {
                #[cfg(feature = "not-recommended-rfs")]
                if rfs == 0 {
                    return Err(Error::InvalidRfs);
                }
                #[cfg(not(feature = "not-recommended-rfs"))]
                if rfs < RECOMMENDED_RFS_MIN || rfs > RECOMMENDED_RFS_MAX {
                    return Err(Error::InvalidRfs);
                }
            }
        }
        Ok(Self {
            i2c,
            rfs0_ohm,
            rfs1_ohm,
        })
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
            Status::SinkMicroAmp(current) => {
                let rfs = match output {
                    Output::Zero => self.rfs0_ohm.ok_or(Error::UnknownRfs)?,
                    Output::One => self.rfs1_ohm.ok_or(Error::UnknownRfs)?,
                };
                ((current * (rfs as f32)) / 62_312.5) as u8
            }
            Status::SourceMicroAmp(current) => {
                let rfs = match output {
                    Output::Zero => self.rfs0_ohm.ok_or(Error::UnknownRfs)?,
                    Output::One => self.rfs1_ohm.ok_or(Error::UnknownRfs)?,
                };
                // ensures MSB is 1
                ((current * (rfs as f32)) / 62_312.5) as u8 | 0x80
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

        let mut status = buf[0].into();
        match output {
            Output::Zero => {
                if let Some(rfs) = self.rfs0_ohm {
                    status = match status {
                        Status::Sink(code) => {
                            Status::SinkMicroAmp(Status::Sink(code).current_ua(rfs).unwrap())
                        }
                        Status::Source(code) => {
                            Status::SourceMicroAmp(Status::Source(code).current_ua(rfs).unwrap())
                        }
                        _ => status,
                    }
                }
            }
            Output::One => {
                if let Some(rfs) = self.rfs1_ohm {
                    status = match status {
                        Status::Sink(code) => {
                            Status::SinkMicroAmp(Status::Sink(code).current_ua(rfs).unwrap())
                        }
                        Status::Source(code) => {
                            Status::SourceMicroAmp(Status::Source(code).current_ua(rfs).unwrap())
                        }
                        _ => status,
                    }
                }
            }
        }
        Ok(status)
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
    fn status_to_code_conversion() {
        assert_eq!(Status::Sink(42).code(), Some(0x2A));
        assert_eq!(Status::Source(42).code(), Some(0x2A));
        assert_eq!(Status::Disable.code(), Some(0x00));
        assert_eq!(Status::Sink(0).code(), Some(0x00));
        assert_eq!(Status::Source(0).code(), Some(0x00));
        assert_eq!(Status::SinkMicroAmp(42.0).code(), None);
        assert_eq!(Status::SourceMicroAmp(42.0).code(), None);
    }

    #[test]
    fn code_to_current_ua_conversion() {
        // example from datasheet
        assert_eq!(Status::Source(42).current_ua(80_000), Some(32.71406));
        assert_eq!(Status::Sink(42).current_ua(80_000), Some(32.71406));
        assert_eq!(Status::Disable.current_ua(1000), Some(0.0));
        assert_eq!(Status::SourceMicroAmp(42.0).current_ua(80_000), None);
        assert_eq!(Status::SinkMicroAmp(42.0).current_ua(80_000), None);
    }

    #[test]
    fn u8_to_status_conversion() {
        assert_eq!(Status::from(0x2A), Status::Sink(42));
        assert_eq!(Status::from(0xAA), Status::Source(42));
        assert_eq!(Status::from(0x00), Status::Disable);
        assert_eq!(Status::from(0x80), Status::Disable);
    }

    #[test]
    fn can_get_output_0_status() {
        let expectations = [i2c::Transaction::write_read(
            SLAVE_ADDRESS,
            vec![Output::Zero as u8],
            vec![0xAA],
        )];
        let mock = i2c::Mock::new(&expectations);
        let mut ds4432 = DS4432::new(mock);

        let status = ds4432.status(Output::Zero).unwrap();
        assert!(matches!(status, Status::Source(42)));

        let mut mock = ds4432.release();
        mock.done();
    }

    #[test]
    fn can_set_output_1_status() {
        let expectations = [i2c::Transaction::write(
            SLAVE_ADDRESS,
            vec![Output::One as u8, 0x2A],
        )];
        let mock = i2c::Mock::new(&expectations);
        let mut ds4432 = DS4432::new(mock);

        // just making sure it doesn't error
        ds4432.set_status(Output::One, Status::Sink(42)).unwrap();

        let mut mock = ds4432.release();
        mock.done();
    }

    #[test]
    fn can_get_output_0_status_current() {
        let expectations = [i2c::Transaction::write_read(
            SLAVE_ADDRESS,
            vec![Output::Zero as u8],
            vec![0xAA],
        )];
        let mock = i2c::Mock::new(&expectations);
        let mut ds4432 = DS4432::with_rfs(mock, Some(80_000), None).unwrap();

        let status = ds4432.status(Output::Zero).unwrap();
        assert!(matches!(status, Status::SourceMicroAmp(32.71406)));

        let mut mock = ds4432.release();
        mock.done();
    }

    #[test]
    fn can_set_output_1_status_current() {
        let expectations = [i2c::Transaction::write(
            SLAVE_ADDRESS,
            vec![Output::One as u8, 0x2A],
        )];
        let mock = i2c::Mock::new(&expectations);
        let mut ds4432 = DS4432::with_rfs(mock, None, Some(80_000)).unwrap();

        // just making sure it doesn't error
        ds4432
            .set_status(Output::One, Status::SinkMicroAmp(32.71406))
            .unwrap();

        let mut mock = ds4432.release();
        mock.done();
    }
}
