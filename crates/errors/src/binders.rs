use crate::{
    AnyDiag, Diag, DiagLike, Kind, RaisedMessage, Raiser, Reportable,
    causes::IntoCause,
    out,
    raiser::RaisedToDiag,
    sealed::{Sealed, SealedToken},
};
pub trait Bind: Sealed + Sized {
    type Out: out::Out;

    #[doc(hidden)]
    fn into_diag<R>(
        self,
        func: impl FnOnce(Raiser<'_>) -> R,
        _: SealedToken,
    ) -> <Self::Out as out::Out>::Ty<R::Diag>
    where
        R: RaisedToDiag;
}

pub(crate) struct ResultBind<T, E> {
    value: Result<T, E>,
    raiser: Raiser<'static>,
}

impl<T, E> ResultBind<T, E> {
    #[track_caller]
    pub(crate) fn new(value: Result<T, E>) -> Self {
        Self {
            value,
            raiser: Raiser::new(),
        }
    }
}

impl<T, E> Sealed for ResultBind<T, E> {}

impl<T, E> Bind for ResultBind<T, E>
where
    E: IntoCause,
{
    type Out = out::Result<T>;

    fn into_diag<R>(self, func: impl FnOnce(Raiser<'_>) -> R, _: SealedToken) -> Result<T, R::Diag>
    where
        R: RaisedToDiag,
    {
        match self.value {
            Ok(value) => Ok(value),
            Err(error) => Err(func(self.raiser).into_diag([error])),
        }
    }
}

pub(crate) struct ErrResultBind<T, E> {
    value: Result<T, E>,
    raiser: Raiser<'static>,
}

impl<T, E> ErrResultBind<T, E> {
    #[track_caller]
    pub(crate) fn new(value: Result<T, E>) -> Self {
        Self {
            value,
            raiser: Raiser::new(),
        }
    }
}

impl<T, E> Sealed for ErrResultBind<T, E> {}

impl<T, E> Bind for ErrResultBind<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    type Out = out::Result<T>;

    fn into_diag<R>(self, func: impl FnOnce(Raiser<'_>) -> R, _: SealedToken) -> Result<T, R::Diag>
    where
        R: RaisedToDiag,
    {
        match self.value {
            Ok(value) => Ok(value),
            Err(error) => Err(func(self.raiser).into_diag_with_err(error)),
        }
    }
}

pub(crate) struct ValueBind<T> {
    value: T,
    raiser: Raiser<'static>,
}

impl<T> ValueBind<T>
where
    T: IntoCause,
{
    #[track_caller]
    pub(crate) fn new(value: T) -> Self {
        Self {
            value,
            raiser: Raiser::new(),
        }
    }
}

impl<T> Sealed for ValueBind<T> {}

impl<T> Bind for ValueBind<T>
where
    T: IntoCause,
{
    type Out = out::Value;

    fn into_diag<R>(self, func: impl FnOnce(Raiser<'_>) -> R, _: SealedToken) -> R::Diag
    where
        R: RaisedToDiag,
    {
        func(self.raiser).into_diag([self.value])
    }
}

pub(crate) struct OptionBind<T> {
    value: Option<T>,
    raiser: Raiser<'static>,
}

impl<T> OptionBind<T> {
    #[track_caller]
    pub(crate) fn new(value: Option<T>) -> Self {
        Self {
            value,
            raiser: Raiser::new(),
        }
    }
}

impl<T> Sealed for OptionBind<T> {}

impl<T> Bind for OptionBind<T> {
    type Out = out::Result<T>;

    fn into_diag<R>(self, func: impl FnOnce(Raiser<'_>) -> R, _: SealedToken) -> Result<T, R::Diag>
    where
        R: RaisedToDiag,
    {
        if let Some(value) = self.value {
            return Ok(value);
        }

        Err(func(self.raiser).into_new_diag())
    }
}

pub trait ContextBind: Sealed {
    type Out;

    #[doc(hidden)]
    fn add_message(
        self,
        msg_fn: impl FnOnce(Raiser<'_>) -> RaisedMessage,
        _: SealedToken,
    ) -> Self::Out;
}

pub(crate) struct ValueContextBind<T> {
    value: T,
    raiser: Raiser<'static>,
}

impl<T> ValueContextBind<T> {
    #[track_caller]
    pub(crate) fn new(value: T) -> Self {
        Self {
            value,
            raiser: Raiser::new(),
        }
    }
}

impl<T> Sealed for ValueContextBind<T> {}

impl<T> ContextBind for ValueContextBind<T>
where
    T: DiagLike,
{
    type Out = T;

    fn add_message(
        mut self,
        msg_fn: impl FnOnce(Raiser<'_>) -> RaisedMessage,
        _: SealedToken,
    ) -> Self::Out {
        self.value.add_context_message(msg_fn(self.raiser));
        self.value
    }
}

pub(crate) struct ResultContextBind<T, E> {
    value: Result<T, E>,
    raiser: Raiser<'static>,
}

impl<T, E> ResultContextBind<T, E> {
    #[track_caller]
    pub(crate) fn new(value: Result<T, E>) -> Self {
        Self {
            value,
            raiser: Raiser::new(),
        }
    }
}

impl<T, E> Sealed for ResultContextBind<T, E> {}

impl<T, E> ContextBind for ResultContextBind<T, E>
where
    E: DiagLike,
{
    type Out = Result<T, E>;

    fn add_message(
        mut self,
        msg_fn: impl FnOnce(Raiser<'_>) -> RaisedMessage,
        _: SealedToken,
    ) -> Self::Out {
        if let Err(err) = &mut self.value {
            err.add_context_message(msg_fn(self.raiser));
        }
        self.value
    }
}

/// An object whose methods bind a context message to a value, and returns that
/// value.
///
/// In the public interface, this is typically used with an `impl` of the
/// `ContextBind` trait. The `Out` type in that binding is the type that will be
/// returned from all methods of this struct.
pub struct ContextBinder<B> {
    binder: B,
}
impl<B: ContextBind> ContextBinder<B> {
    pub(crate) fn new(binder: B) -> Self {
        Self { binder }
    }

    /// Binds a reportable message of type `M` to a diag-like, and returns the
    /// value.
    pub fn msg<M>(self, msg: M) -> B::Out
    where
        M: Reportable,
    {
        self.binder.add_message(|r| r.msg(msg), SealedToken)
    }

    /// Binds a [`std::format_args!`] printable string to a diag-like, and
    /// returns the value.
    pub fn args(self, args: std::fmt::Arguments<'_>) -> B::Out {
        self.binder.add_message(|r| r.args(args), SealedToken)
    }
}

pub struct RaiseBinder<B: Bind> {
    binder: B,
}

impl<B: Bind> RaiseBinder<B> {
    pub(crate) fn new(binder: B) -> Self {
        Self { binder }
    }

    pub fn kind<K>(self, kind: K) -> <B::Out as out::Out>::Ty<Diag<K>>
    where
        K: Kind + Reportable,
    {
        self.binder.into_diag(move |r| r.kind(kind), SealedToken)
    }

    pub fn kind_msg<K, M>(self, kind: K, msg: M) -> <B::Out as out::Out>::Ty<Diag<K>>
    where
        K: Kind,
        M: Reportable,
    {
        self.binder
            .into_diag(move |r| r.kind_msg(kind, msg), SealedToken)
    }

    pub fn kind_args<K>(
        self,
        kind: K,
        args: std::fmt::Arguments<'_>,
    ) -> <B::Out as out::Out>::Ty<Diag<K>>
    where
        K: Kind,
    {
        self.binder
            .into_diag(move |r| r.kind_args(kind, args), SealedToken)
    }

    pub fn msg<M>(self, msg: M) -> <B::Out as out::Out>::Ty<AnyDiag>
    where
        M: Reportable,
    {
        self.binder.into_diag(move |r| r.msg(msg), SealedToken)
    }

    pub fn args(self, args: std::fmt::Arguments<'_>) -> <B::Out as out::Out>::Ty<AnyDiag> {
        self.binder.into_diag(move |r| r.args(args), SealedToken)
    }
}
