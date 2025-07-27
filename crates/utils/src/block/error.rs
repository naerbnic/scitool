use std::{any::Any, io};

fn try_cast_to<Target, T>(value: T) -> Result<Target, T>
where
    T: 'static,
    Target: 'static,
{
    match (Box::new(value) as Box<dyn Any>).downcast::<Target>() {
        Ok(target) => Ok(*target),
        Err(value) => Err(*value.downcast::<T>().unwrap()),
    }
}

/// An error that occurs while loading a block value.
#[derive(thiserror::Error)]
#[error(transparent)]
pub struct ReadError(io::Error);

impl std::fmt::Debug for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.0, f)
    }
}

impl ReadError {
    /// Create a new error from an [`io::Error`].
    #[must_use]
    pub fn new(err: io::Error) -> Self {
        Self(err)
    }

    /// Create a new error from an implementation of [`std::error::Error`].
    pub fn from_std_err<E>(err: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        // This should get optimized away.
        match try_cast_to(err) {
            Ok(io_err) => Self(io_err),
            Err(err) => Self(io::Error::other(err)),
        }
    }
}

impl From<ReadError> for io::Error {
    fn from(err: ReadError) -> Self {
        err.0
    }
}

impl From<io::Error> for ReadError {
    fn from(err: io::Error) -> Self {
        Self(err)
    }
}

/// The result of a read operation.
pub type ReadResult<T> = std::result::Result<T, ReadError>;
