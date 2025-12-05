use std::{
    any::{Any, TypeId},
    collections::HashMap,
    io,
    sync::{LazyLock, RwLock},
};

use crate::utils::errors::{BoxError, DynError};

static WRAPPER_REGISTRY: LazyLock<RwLock<WrapCastHandlerRegistry>> = LazyLock::new(|| {
    let mut registry = WrapCastHandlerRegistry::new();
    registry.register::<std::io::Error>();
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

struct UnwrapResult {
    error: BoxError,
    did_unwrap: bool,
}

trait WrapCastHandler {
    fn unwrap_boxed(&self, err: BoxError) -> UnwrapResult;
}

struct WrapCastHandlerImpl<W>(std::marker::PhantomData<W>);

impl<W> WrapCastHandler for WrapCastHandlerImpl<W>
where
    W: ErrWrapper,
{
    fn unwrap_boxed(&self, err: BoxError) -> UnwrapResult {
        let mut did_unwrap = false;
        let error = match err.downcast::<W>() {
            Ok(io_err) => match io_err.try_unwrap_box() {
                Ok(inner) => {
                    did_unwrap = true;
                    inner
                }
                Err(wrap) => Box::new(wrap),
            },
            Err(original) => original,
        };

        UnwrapResult { error, did_unwrap }
    }
}

struct WrapCastHandlerRegistry {
    handlers: HashMap<TypeId, Box<dyn WrapCastHandler + Send + Sync>>,
}

impl WrapCastHandlerRegistry {
    fn new() -> Self {
        WrapCastHandlerRegistry {
            handlers: HashMap::new(),
        }
    }

    fn register<W>(&mut self)
    where
        W: ErrWrapper + Send + Sync + 'static,
    {
        self.handlers
            .entry(TypeId::of::<W>())
            .or_insert_with(|| Box::new(WrapCastHandlerImpl::<W>(std::marker::PhantomData)));
    }

    fn resolve(&self, mut err: BoxError) -> BoxError {
        loop {
            let type_id = Any::type_id(&*err);
            let Some(handler) = self.handlers.get(&type_id) else {
                return err;
            };
            let UnwrapResult { error, did_unwrap } = handler.unwrap_boxed(err);
            if !did_unwrap {
                return error;
            }
            err = error;
        }
    }
}

pub(crate) fn register_wrapper<W>()
where
    W: ErrWrapper + Send + Sync + 'static,
{
    let mut registry = WRAPPER_REGISTRY
        .write()
        .expect("Failed to acquire write lock on wrapper registry");
    registry.register::<W>();
}

pub(crate) fn resolve_error(err: BoxError) -> BoxError {
    let registry = WRAPPER_REGISTRY
        .read()
        .expect("Failed to acquire read lock on wrapper registry");
    registry.resolve(err)
}

macro_rules! once_registerer {
    ($(#[$m:meta])* $v:vis fn $name:ident($t:ty)) => {
        $(#[$m])* $v fn $name() {
            static ONCE: std::sync::Once = std::sync::Once::new();
            ONCE.call_once(|| {
                $crate::utils::errors::unpack::register_wrapper::<$t>();
            });
        }
    };
}

pub(crate) use once_registerer;
