macro_rules! io_err {
   ($kind:ident, $fmt:literal $($arg:tt)*) => {
       std::io::Error::new(std::io::ErrorKind::$kind, format!($fmt $($arg)*))
   };
}

macro_rules! io_bail {
   ($kind:ident, $fmt:literal $($arg:tt)*) => {
       return Err($crate::err_helpers::io_err!($kind, $fmt $($arg)*))
   };
}

pub(crate) use {io_bail, io_err};
