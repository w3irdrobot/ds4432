/// Driver Result type.
pub type Result<T, E> = core::result::Result<T, Error<E>>;

/// Driver errors.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum Error<E> {
    /// I2C bus error.
    I2c(E),
    /// The given code is too high
    InvalidCode(u8),
    /// The given Iout is out of range
    InvalidIout,
    /// The given RFS is out of range
    InvalidRfs,
    /// Try to set a Current value without giving the Rfs value
    UnknownRfs,
}

#[cfg(feature = "core-error")]
impl<E: core::fmt::Debug> core::error::Error for Error<E> {}

#[cfg(feature = "core-error")]
impl<E: core::fmt::Debug> core::fmt::Display for Error<E> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}
