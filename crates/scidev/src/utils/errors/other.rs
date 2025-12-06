//! A generic error type for ergonomic error handling/reporting.
#![allow(clippy::disallowed_types)] // Allow anyhow usage in this module only

use std::any::Any;
use std::borrow::Cow;
use std::fmt::Debug;
use std::fmt::Display;
use std::io;

use crate::utils::errors::{BoxError, DynError, ErrWrapper, once_registerer, resolve_error};

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

once_registerer!(fn register_other(OtherError));

enum OtherKind {
    Wrapped,
    Context,
}

pub(crate) struct OtherError {
    kind: OtherKind,
    error: anyhow::Error,
}

impl OtherError {
    pub(crate) fn new<E>(err: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        register_other();
        match try_downcast::<OtherError, E>(err) {
            Ok(other) => other,
            Err(e) => OtherError {
                kind: OtherKind::Wrapped,
                error: anyhow::Error::new(e),
            },
        }
    }

    pub(crate) fn from_boxed(err: BoxError) -> Self {
        register_other();
        OtherError {
            kind: OtherKind::Wrapped,
            error: anyhow::Error::from_boxed(err),
        }
    }

    pub(crate) fn from_msg<M>(msg: M) -> Self
    where
        M: Display + Debug + Send + Sync + 'static,
    {
        register_other();
        OtherError {
            kind: OtherKind::Context,
            error: anyhow::Error::msg(msg),
        }
    }
}

impl From<io::Error> for OtherError {
    fn from(err: io::Error) -> Self {
        OtherError::from_boxed(resolve_error(Box::new(err)))
    }
}

impl From<Box<dyn std::error::Error + Send + Sync>> for OtherError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
        OtherError::from_boxed(err)
    }
}

impl Display for OtherError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        Display::fmt(&self.error, f)
    }
}

impl Debug for OtherError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        Debug::fmt(&self.error, f)
    }
}

impl std::error::Error for OtherError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.error.source()
    }
}

impl ErrWrapper for OtherError {
    fn wrapped_err(&self) -> Option<&DynError> {
        if let OtherKind::Wrapped = self.kind {
            Some(self.error.as_ref())
        } else {
            None
        }
    }

    fn wrap_box(err: BoxError) -> Self {
        OtherError::from_boxed(err)
    }

    fn try_unwrap_box(self) -> Result<Box<dyn std::error::Error + Send + Sync>, Self> {
        if self.is_wrapping() {
            let boxed = self.error.into_boxed_dyn_error();
            Ok(boxed)
        } else {
            Err(self)
        }
    }
}

pub(crate) trait ResultExt<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn with_other_err(self) -> Result<T, OtherError>;
}

impl<T, E> ResultExt<T, E> for Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn with_other_err(self) -> Result<T, OtherError> {
        self.map_err(OtherError::new)
    }
}

pub(crate) trait OptionExt<T> {
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
