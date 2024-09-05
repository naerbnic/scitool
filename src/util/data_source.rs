pub mod io_source;

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
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<(), Error>;
    fn size(&self) -> Result<u64, Error>;
}
