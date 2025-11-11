pub(super) trait TryIntoIoError: Sized {
    fn try_into_io_err(self) -> Result<std::io::Error, Self>;
}

pub(super) trait WrapIoError: Sized {
    fn wrap_io_err(err: std::io::Error) -> Self;
}

pub(super) trait IntoIoError: Sized {
    fn into_io_err(self) -> std::io::Error;
}

impl<T> IntoIoError for T
where
    T: TryIntoIoError + std::error::Error + Send + Sync + 'static,
{
    fn into_io_err(self) -> std::io::Error {
        match self.try_into_io_err() {
            Ok(io_err) => io_err,
            Err(other) => std::io::Error::other(other),
        }
    }
}

pub(super) trait FromIoError: Sized {
    fn from_io_err(err: std::io::Error) -> Self;
}
