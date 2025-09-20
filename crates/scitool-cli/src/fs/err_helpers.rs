#[derive(Debug, thiserror::Error)]
#[error("{context}: {error}")]
pub struct ErrorWithContext<T, E> {
    context: T,
    #[source]
    error: E,
}

impl<T, E> ErrorWithContext<T, E>
where
    T: std::fmt::Display,
    E: std::error::Error,
{
    pub fn new(context: T, error: E) -> Self {
        Self { context, error }
    }
}

macro_rules! io_err {
   ($kind:ident, $fmt:literal $($arg:tt)*) => {
       std::io::Error::new(std::io::ErrorKind::$kind, format!($fmt $($arg)*))
   };
}

macro_rules! io_err_map {
    ($kind:ident, $fmt:literal $($arg:tt)+) => {
        |e| std::io::Error::new(std::io::ErrorKind::$kind, $crate::fs::err_helpers::ErrorWithContext::new(format!($fmt $($arg)+), e))
    };
    ($kind:ident, $fmt:literal) => {
        |e| std::io::Error::new(std::io::ErrorKind::$kind, $crate::fs::err_helpers::ErrorWithContext::new($fmt, e))
    };
    ($kind:ident) => {
        |e| std::io::Error::new(std::io::ErrorKind::$kind, e)
    };
}

macro_rules! io_bail {
   ($kind:ident, $fmt:literal $($arg:tt)*) => {
       return Err($crate::fs::err_helpers::io_err!($kind, $fmt $($arg)*))
   };
}

macro_rules! io_async_bail {
   ($kind:ident, $fmt:literal $($arg:tt)*) => {
       return std::task::Poll::Ready(Err($crate::fs::err_helpers::io_err!($kind, $fmt $($arg)*)));
   };
}

pub(crate) use {io_async_bail, io_bail, io_err, io_err_map};
