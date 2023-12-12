//! Traits

/// Trait to implement for entities implementing Iterator.
pub trait TryNext {
    /// Item type
    type Item;
    /// Error type
    type Error;

    /// Try to advance the iterator, returning the next value or an error indicating that no value was found
    fn try_next(&mut self) -> Result<Self::Item, Self::Error>;
}

/// Trait to implement for entities implementing Iterator.
pub trait TryNextError {
    /// Error type
    type Error;

    /// Return an error indicating that no value was found
    fn try_next_err(&mut self) -> Self::Error;
}

impl<T> TryNext for T
where
    T: Iterator + TryNextError,
{
    type Item = <T as Iterator>::Item;
    type Error = T::Error;

    fn try_next(&mut self) -> Result<Self::Item, Self::Error> {
        self.next().ok_or_else(|| self.try_next_err())
    }
}
