use std::{
    any::{Any, TypeId},
    collections::HashMap,
    io,
    sync::{LazyLock, RwLock},
};

use crate::utils::errors::{BoxError, DynError, other::OtherError};

static WRAPPER_REGISTRY: LazyLock<RwLock<WrapCastHandlerRegistry>> = LazyLock::new(|| {
    let mut registry = WrapCastHandlerRegistry::new();
    registry.register_wrapper::<std::io::Error>();
    registry.register_wrapper::<OtherError>();
    RwLock::new(registry)
});

/// A trait for error types that can wrap other generic errors.
///
/// These types can either have their own "primitive" error variants, or
/// simply wrap another error without adding any context.
pub(crate) trait ErrWrapper: std::error::Error + Send + Sync + 'static + Sized {
    /// Returns a reference to the inner wrapped error, if one is wrapped.
    fn wrapped_err(&self) -> Option<&DynError>;

    /// Wraps the given error into this type. The resulting error should
    /// return true from `is_wrapping()`.
    fn wrap_box(err: BoxError) -> Self;

    /// Attempts to unwrap this error into the inner wrapped error. If this
    /// error is not wrapping another error, returns `Err(self)`.
    ///
    /// Should always return `Ok` for types where `is_wrapping()` returns true.
    fn try_unwrap_box(self) -> Result<BoxError, Self>;

    /// Returns true iff this type is wrapping another error without any
    /// additional context.
    ///
    /// If this is true, there must exist some type `E` such that
    /// `Self::wrap(E)` produces this error.
    fn is_wrapping(&self) -> bool {
        self.wrapped_err().is_some()
    }
}

impl ErrWrapper for io::Error {
    fn wrap_box(err: BoxError) -> Self {
        io::Error::other(err)
    }

    fn wrapped_err(&self) -> Option<&DynError> {
        if matches!(self.kind(), io::ErrorKind::Other) && self.get_ref().is_some() {
            self.get_ref()
        } else {
            None
        }
    }

    fn try_unwrap_box(self) -> Result<BoxError, Self> {
        if self.is_wrapping() {
            let boxed = self.into_inner().unwrap();
            Ok(boxed)
        } else {
            Err(self)
        }
    }

    fn is_wrapping(&self) -> bool {
        self.wrapped_err().is_some()
    }
}

type DynUnwrapFn = dyn Fn(BoxError) -> UnwrapResult + Send + Sync;
type BoxUnwrapFn = Box<DynUnwrapFn>;

struct UnwrapResult {
    error: BoxError,
    did_unwrap: bool,
}

fn cast_or_panic<T: std::error::Error + Send + Sync + 'static>(value: BoxError) -> T {
    *value.downcast::<T>().expect("Failed to downcast boxed Any")
}

fn unwrap_boxed<W>(err: W) -> UnwrapResult
where
    W: ErrWrapper,
{
    let mut did_unwrap = false;
    let error = match err.try_unwrap_box() {
        Ok(inner) => {
            did_unwrap = true;
            inner
        }
        Err(wrap) => Box::new(wrap),
    };

    UnwrapResult { error, did_unwrap }
}

struct WrapCastHandlerRegistry {
    handlers: HashMap<TypeId, BoxUnwrapFn>,
}

impl WrapCastHandlerRegistry {
    fn new() -> Self {
        WrapCastHandlerRegistry {
            handlers: HashMap::new(),
        }
    }

    fn register_wrapper<W>(&mut self)
    where
        W: ErrWrapper + Send + Sync + 'static,
    {
        self.handlers
            .entry(TypeId::of::<W>())
            .or_insert_with(|| Box::new(|err| unwrap_boxed::<W>(cast_or_panic::<W>(err))));
    }

    fn resolve(&self, mut err: BoxError) -> BoxError {
        loop {
            let type_id = Any::type_id(&*err);
            let Some(handler) = self.handlers.get(&type_id) else {
                return err;
            };
            let UnwrapResult { error, did_unwrap } = handler(err);
            if !did_unwrap {
                return error;
            }
            err = error;
        }
    }
}

pub(crate) fn resolve_error(err: BoxError) -> BoxError {
    let registry = WRAPPER_REGISTRY
        .read()
        .expect("Failed to acquire read lock on wrapper registry");
    registry.resolve(err)
}
