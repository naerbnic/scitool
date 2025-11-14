//! A generic error type for ergonomic error handling/reporting.
#![allow(clippy::disallowed_types)] // Allow anyhow usage in this module only

use std::any::Any;
use std::any::TypeId;
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::Debug;
use std::fmt::Display;
use std::io;

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

pub(crate) type DynError = dyn std::error::Error + Send + Sync + 'static;
pub(crate) type BoxError = Box<DynError>;

enum OtherKind {
    Wrapped,
    Context,
}

pub struct OtherError {
    kind: OtherKind,
    error: anyhow::Error,
}

impl OtherError {
    pub fn new<E>(err: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        match try_downcast::<OtherError, E>(err) {
            Ok(other) => other,
            Err(e) => OtherError {
                kind: OtherKind::Wrapped,
                error: anyhow::Error::new(e),
            },
        }
    }

    pub fn from_boxed(err: BoxError) -> Self {
        OtherError {
            kind: OtherKind::Wrapped,
            error: anyhow::Error::from_boxed(err),
        }
    }

    pub fn from_wrapper<W>(wrapper: W) -> Self
    where
        W: ErrWrapper,
    {
        match wrapper.try_unwrap_box() {
            Ok(other) => OtherError {
                kind: OtherKind::Wrapped,
                error: anyhow::Error::from_boxed(other),
            },
            Err(wrap) => OtherError::new(wrap),
        }
    }

    pub fn from_msg<M>(msg: M) -> Self
    where
        M: Display + Debug + Send + Sync + 'static,
    {
        OtherError {
            kind: OtherKind::Context,
            error: anyhow::Error::msg(msg),
        }
    }

    pub fn add_context(self, msg: String) -> Self {
        OtherError {
            kind: OtherKind::Context,
            error: self.error.context(msg),
        }
    }

    pub fn downcast<Target>(self) -> Result<Target, Self>
    where
        Target: Display + Debug + Send + Sync + 'static,
    {
        if let OtherKind::Context = self.kind {
            return Err(self);
        }
        match self.error.downcast::<Target>() {
            Ok(downcasted) => Ok(downcasted),
            Err(original) => Err(OtherError {
                kind: OtherKind::Wrapped,
                error: original,
            }),
        }
    }
}

impl From<io::Error> for OtherError {
    fn from(err: io::Error) -> Self {
        CastChain::new(err)
            .register_wrapper::<OtherError>()
            .with_cast(OtherError::new::<io::Error>)
            .finish(|e| e)
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

/// A trait for error types that can wrap other generic errors.
/// 
/// These types can either have their own "primitive" error variants, or
/// simply wrap another error without adding any context.
pub trait ErrWrapper: std::error::Error + Send + Sync + 'static + Sized {
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

    /// Returns the `TypeId` of the inner wrapped error, if one is wrapped.
    fn wrapped_type_id(&self) -> Option<TypeId> {
        self.wrapped_err().map(Any::type_id)
    }

    /// Wraps the given error into this type. The resulting error should
    /// return true from `is_wrapping()`.
    fn wrap<E>(err: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::wrap_box(Box::new(err))
    }

    /// Attempts to downcast this error into the given target type. If this
    /// error is wrapping another error, the inner error will be downcasted.
    fn downcast<Target>(self) -> Result<Target, Self>
    where
        Target: std::error::Error + Send + Sync + 'static,
    {
        match self.try_unwrap_box() {
            Ok(boxed) => match boxed.downcast::<Target>() {
                Ok(downcasted) => Ok(*downcasted),
                Err(original) => Err(Self::wrap_box(original)),
            },
            Err(wrap) => Err(wrap),
        }
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

    fn downcast<Target>(self) -> Result<Target, Self>
    where
        Target: std::error::Error + Send + Sync + 'static,
    {
        self.downcast()
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

trait WrapCastHandler {
    fn unwrap_boxed(&self, err: BoxError) -> Result<BoxError, BoxError>;
}

#[derive(Default)]
struct WrapCastHandlerImpl<W>(std::marker::PhantomData<W>);

impl<W> WrapCastHandler for WrapCastHandlerImpl<W>
where
    W: ErrWrapper,
{
    fn unwrap_boxed(&self, err: BoxError) -> Result<BoxError, BoxError> {
        match err.downcast::<W>() {
            Ok(io_err) => match io_err.try_unwrap_box() {
                Ok(inner) => Ok(inner),
                Err(wrap) => Err(Box::new(wrap)),
            },
            Err(original) => Err(original),
        }
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
        self.handlers.insert(
            TypeId::of::<W>(),
            Box::new(WrapCastHandlerImpl::<W>(std::marker::PhantomData)),
        );
    }

    fn resolve(&self, mut err: BoxError) -> BoxError {
        loop {
            let type_id = Any::type_id(&*err);
            let Some(handler) = self.handlers.get(&type_id) else {
                return err;
            };
            match handler.unwrap_boxed(err) {
                Ok(unwrapped) => err = unwrapped,
                Err(wrap) => return wrap,
            }
        }
    }
}

enum CastChainState<WrapE, E> {
    Registration {
        registry: WrapCastHandlerRegistry,
        wrap: WrapE,
    },
    HasWrap(BoxError),
    ResolvedError(E),
}

pub(crate) struct CastChain<WrapE, E> {
    state: CastChainState<WrapE, E>,
}

impl<WrapE, E> CastChain<WrapE, E>
where
    WrapE: ErrWrapper,
    E: std::error::Error + Send + Sync + 'static,
{
    pub(crate) fn new(wrap: WrapE) -> Self {
        let mut registry = WrapCastHandlerRegistry::new();
        registry.register::<WrapE>();

        CastChain {
            state: CastChainState::Registration { registry, wrap },
        }
    }

    pub(crate) fn register_wrapper<W>(mut self) -> Self
    where
        W: ErrWrapper,
    {
        match &mut self.state {
            CastChainState::Registration { registry, .. } => {
                registry.register::<W>();
            }
            _ => panic!("Cannot register new wrapper after calling with_cast"),
        }
        self
    }

    pub(crate) fn with_cast<E2>(mut self, map: impl FnOnce(E2) -> E) -> Self
    where
        E2: std::error::Error + Send + Sync + 'static,
    {
        self = self.resolve_registry();
        match self.state {
            CastChainState::Registration { .. } => unreachable!(),
            CastChainState::HasWrap(other) => {
                self.state = match other.downcast() {
                    Ok(err) => CastChainState::ResolvedError(map(*err)),
                    Err(wrap) => CastChainState::HasWrap(wrap),
                }
            }
            CastChainState::ResolvedError(_) => {}
        }
        self
    }

    pub(crate) fn finish<WrapErr2>(mut self, map: impl FnOnce(WrapErr2) -> E) -> E
    where
        WrapErr2: ErrWrapper,
    {
        self = self.resolve_registry();
        match self.state {
            CastChainState::Registration { .. } => unreachable!(),
            CastChainState::HasWrap(wrap) => map(WrapErr2::wrap_box(wrap)),
            CastChainState::ResolvedError(err) => err,
        }
    }

    fn resolve_registry(mut self) -> Self {
        if let CastChainState::Registration { registry, wrap } = self.state {
            let wrap = registry.resolve(Box::new(wrap));
            self.state = CastChainState::HasWrap(wrap);
        }
        self
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
