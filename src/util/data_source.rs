pub mod bounded;
pub mod io_source;
pub mod cloneable;

pub struct Error(anyhow::Error);

impl Error {
    pub fn try_downcast<T: std::error::Error + Send + Sync + 'static>(&self) -> Option<&T> {
        self.0.downcast_ref()
    }

    pub fn try_as_io_error(&self) -> Option<&std::io::Error> {
        self.try_downcast()
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl<T> From<T> for Error
where
    T: Into<anyhow::Error>,
{
    fn from(err: T) -> Self {
        Error(err.into())
    }
}

impl From<Error> for std::io::Error {
    fn from(err: Error) -> Self {
        let non_io_error = match err.0.downcast() {
            Ok(err) => return err,
            Err(err) => err,
        };
        let inner_error: Box<dyn std::error::Error + Send + Sync> = non_io_error.into();
        std::io::Error::new(std::io::ErrorKind::Other, inner_error)
    }
}

pub trait DataSource {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<(), Error>;
}

impl<D> DataSource for Box<D>
where
    D: DataSource + ?Sized,
{
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<(), Error> {
        (**self).read_at(offset, buf)
    }
}

pub trait BoundedDataSource: DataSource {
    fn size(&mut self) -> Result<u64, Error>;
}

impl<D> BoundedDataSource for Box<D>
where
    D: BoundedDataSource + ?Sized,
{
    fn size(&mut self) -> Result<u64, Error> {
        (**self).size()
    }
}

/// Represents a data buffer that can be written to.
///
/// In the abstract, the target can be treated as an array of arbitrary size.
/// This trait does not place any limits on the offset that can be written to.
/// The underlying implementation may impose limits, which will cause errors to
/// be returned when attempting to write past the end of a target, but it
/// is also allowed to automatically increase the size of the underlying target
/// to accommodate the write.
pub trait DataTarget {
    /// Writes the contents of the buffer to the target at the specified offset.
    ///
    /// It is valid to write an empty buffer at an offset. Implementations may
    /// interpret this as extending the target to the specified offset.
    fn write_at(&mut self, offset: u64, buf: &[u8]) -> Result<(), Error>;
}

/// Represents a writable data buffer with a fixed size.
///
/// Any attempts to write_at a range that exceeds the size of the target must
/// result in an UnexpectedEof error.
pub trait BoundedDataTarget: DataTarget {
    fn size(&mut self) -> Result<u64, Error>;
}