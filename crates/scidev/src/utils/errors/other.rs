//! A generic error type for ergonomic error handling/reporting.

use std::any::Any;
use std::borrow::Cow;
use std::fmt::Debug;
use std::fmt::Display;

fn try_downcast<Target: 'static, T: 'static>(value: T) -> Result<Target, T> {
    let value_ref: &dyn Any = &value;
    if value_ref.is::<Target>() {
        // Use a box trick to do the conversion safely. This _should_ optimize away.
        let boxed: Box<dyn Any> = Box::new(value);
        match boxed.downcast::<Target>() {
            Ok(downcasted) => Ok(*downcasted),
            Err(_) => unreachable!("Initial check failed"),
        }
    } else {
        Err(value)
    }
}

pub struct OtherError(anyhow::Error);

impl OtherError {
    pub fn new<E>(err: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        match try_downcast::<OtherError, E>(err) {
            Ok(other) => other,
            Err(e) => OtherError(anyhow::Error::new(e)),
        }
    }

    pub fn from_msg<M>(msg: M) -> Self
    where
        M: Display + Debug + Send + Sync + 'static,
    {
        OtherError(anyhow::Error::msg(msg))
    }

    pub fn add_context(self, msg: String) -> Self {
        OtherError(self.0.context(msg))
    }
}

impl Display for OtherError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Debug for OtherError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl std::error::Error for OtherError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

pub trait OtherMapper {
    type Error;
    fn map_other(self, other: OtherError) -> Self::Error;
}

pub trait ResultExt<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn with_other_err(self) -> Result<T, OtherError>;
    fn with_other_context<'a, M>(self, msg: M) -> Result<T, OtherError>
    where
        M: Into<Cow<'a, str>>;

    fn map_other<M>(self, mapper: M) -> Result<T, M::Error>
    where
        M: OtherMapper<Error = E>;
}

impl<T, E> ResultExt<T, E> for Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn with_other_err(self) -> Result<T, OtherError> {
        self.map_err(OtherError::new)
    }

    fn with_other_context<'a, M>(self, msg: M) -> Result<T, OtherError>
    where
        M: Into<Cow<'a, str>>,
    {
        self.map_err(move |e| OtherError::new(e).add_context(msg.into().into_owned()))
    }

    fn map_other<M>(self, mapper: M) -> Result<T, M::Error>
    where
        M: OtherMapper<Error = E>,
    {
        match self {
            Ok(value) => Ok(value),
            Err(other) => Err(mapper.map_other(OtherError::new(other))),
        }
    }
}

pub trait OptionExt<T> {
    fn ok_or_else_other<F, M>(self, body: F) -> Result<T, OtherError>
    where
        F: FnOnce() -> M,
        M: Into<Cow<'static, str>>;
}

impl<T> OptionExt<T> for Option<T> {
    fn ok_or_else_other<F, M>(self, body: F) -> Result<T, OtherError>
    where
        F: FnOnce() -> M,
        M: Into<Cow<'static, str>>,
    {
        self.ok_or_else(|| OtherError::from_msg(body().into()))
    }
}

macro_rules! ensure_other {
    ($cond:expr, $msg:literal, $($arg:expr),*) => {
        if !$cond {
            return Err(OtherError::from_msg(format!($msg, $($arg),*)).into());
        }
    };
    ($cond:expr, $msg:literal) => {
        if !$cond {
            return Err(OtherError::from_msg($msg).into());
        }
    };
}

macro_rules! bail_other {
    ($msg:literal, $($arg:expr),*) => {
        return Err(OtherError::from_msg(format!($msg, $($arg),*)).into())
    };
    ($msg:literal) => {
        return Err(OtherError::from_msg($msg).into())
    };
}

pub(crate) use {bail_other, ensure_other};
