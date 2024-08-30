/// Driver Result type.
pub type Result<T, E> = core::result::Result<T, Error<E>>;

/// Driver errors.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum Error<E> {
    /// I2C bus error.
    I2c(E),
    /// The given level is too high
    InvalidLevel(u8),
}
