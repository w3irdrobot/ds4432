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
    /// The given RFS cannot be zero
    InvalidRfs,
    /// Try to set a Current value without giving the Rfs value
    UnknownRfs,
}
