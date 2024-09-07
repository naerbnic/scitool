pub mod cloneable;
pub mod io_source;
pub mod subrange;

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

pub type Result<T> = std::result::Result<T, Error>;

pub trait DataBlock {
    fn size(&mut self) -> Result<u64>;
}

impl<D> DataBlock for Box<D>
where
    D: DataBlock + ?Sized,
{
    fn size(&mut self) -> Result<u64> {
        (**self).size()
    }
}

pub trait ReadBlock: DataBlock {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<()>;
}

impl<D> ReadBlock for Box<D>
where
    D: ReadBlock + ?Sized,
{
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<()> {
        (**self).read_at(offset, buf)
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
pub trait WriteBlock: DataBlock {
    /// Writes the contents of the buffer to the target at the specified offset.
    ///
    /// It is valid to write an empty buffer at an offset. Implementations may
    /// interpret this as extending the target to the specified offset.
    fn write_at(&mut self, offset: u64, buf: &[u8]) -> Result<()>;
}

/// Represents a source of data blocks.
pub trait BlockSource {
    fn open_read(&self) -> Result<Box<dyn ReadBlock>>;
    fn open_write(&self) -> Result<Box<dyn WriteBlock>>;
}
