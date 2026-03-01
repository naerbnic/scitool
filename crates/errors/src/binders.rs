use std::panic::Location;

use crate::{
    AnyDiag, Diag, DiagLike, Kind, Raiser, Reportable,
    ext::RaisedToDiag,
    finding::{FindingToRaised, KindFinding, MessageFinding},
    frame::Frame,
};

struct StdErrorCause<T> {
    error: T,
}

impl<T> std::fmt::Debug for StdErrorCause<T>
where
    T: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}

impl<T> std::fmt::Display for StdErrorCause<T>
where
    T: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}

impl<T> Kind for StdErrorCause<T> where T: std::error::Error + Send + Sync + 'static {}

/// An opaque type representing a valid cause for a [`Diag`].
pub struct Cause(Frame);

impl Cause {
    pub(crate) fn from_frame(frame: Frame) -> Self {
        Cause(frame)
    }

    pub(crate) fn into_frame(self) -> Frame {
        self.0
    }
}

pub trait IntoCause: Sized {
    #[doc(hidden)]
    fn into_cause(self, created_at: &'static Location<'static>) -> Cause;
}

impl<T> IntoCause for T
where
    T: std::error::Error + Send + Sync + 'static,
{
    #[track_caller]
    fn into_cause(self, created_at: &'static Location<'static>) -> Cause {
        let cause = Diag::from_finding_and_frames(
            KindFinding::new_kind(StdErrorCause { error: self }),
            vec![],
            created_at,
        );
        cause.into_cause(created_at)
    }
}

impl IntoCause for Cause {
    fn into_cause(self, _created_at: &'static Location<'static>) -> Cause {
        self
    }
}

trait IntoOptCause: Sized {
    fn into_opt_cause(self, created_at: &'static Location<'static>) -> Option<Cause>;
}

impl<T> IntoOptCause for T
where
    T: IntoCause,
{
    fn into_opt_cause(self, created_at: &'static Location<'static>) -> Option<Cause> {
        Some(self.into_cause(created_at))
    }
}

pub(crate) struct NullCause;

impl IntoOptCause for NullCause {
    fn into_opt_cause(self, _created_at: &'static Location<'static>) -> Option<Cause> {
        None
    }
}

struct InnerBinder<R> {
    mapper: R,
    raiser: Raiser<'static>,
}

impl<M> InnerBinder<M>
where
    M: Mapper,
{
    #[track_caller]
    fn new(mapper: M) -> Self {
        Self {
            mapper,
            raiser: Raiser::new(),
        }
    }

    fn add_message(self, msg_fn: MessageFinding) -> M::Out<M::In>
    where
        M::In: DiagLike,
    {
        let Self { mapper, raiser } = self;
        mapper.map_value(move |err| raiser.msg_finding(msg_fn).add_as_context(err))
    }

    fn into_diag<R>(self, func: impl FnOnce() -> R) -> M::Out<<R::Raised as RaisedToDiag>::Diag>
    where
        M::In: IntoOptCause,
        R: FindingToRaised,
    {
        let Self { mapper, raiser } = self;
        let created_at = raiser.created_at();
        mapper.map_value(|err| {
            func()
                .into_raised(raiser)
                .into_diag(err.into_opt_cause(created_at))
        })
    }
}

macro_rules! define_context_binder {
    (
        $(#[$meta:meta])*
        $v:vis struct $name:ident$(<$($ty_var:ident),*>)?($mapper_ty:ty => $out_ty:ty, Docs => {
            $(#[$msg_doc:meta])* msg,
            $(#[$args_doc:meta])* args,
        })
        $(where $($where_clause:tt)*)?
    ) => {
        $(#[$meta])*
        $v struct $name$(<$($ty_var),*>)?(InnerBinder<$mapper_ty>);

        impl$(<$($ty_var),*>)? $name$(<$($ty_var),*>)?
        $(where $($where_clause)*)?
        {
            #[track_caller]
            pub(crate) fn new(mapper: $mapper_ty) -> Self {
                Self(InnerBinder::new(mapper))
            }

            $(#[$msg_doc])*
            $v fn msg<M>(self, msg: M) -> $out_ty
            where
                M: Reportable,
            {
                self.0.add_message(MessageFinding::new_msg(msg))
            }

            $(#[$args_doc])*
            $v fn args(self, args: std::fmt::Arguments<'_>) -> $out_ty {
                self.0.add_message(MessageFinding::new_args(args))
            }
        }
    };
}

define_context_binder! {
    // Note: This does not use the _with methods, as they would be immediately
    // evaluated otherwise.
    pub struct ContextBinder<T>(ValueMapper<T> => T, Docs => {
        /// Adds the given [`Reportable`] message to the error as context.
        msg,
        /// Adds the given [`std::fmt::Arguments`] to the error as context.
        ///
        /// This can be used with [`std::format_args!`] to create a message
        /// from a format string and arguments.
        args,
    }) where T: DiagLike
}

define_context_binder! {
    pub struct ResultContextBinder<T, E>(Result<T, E> => Result<T, E>, Docs => {
        /// If this is a [`Result::Err`], adds the given [`Reportable`] message to the
        /// error as context.
        ///
        /// ```text
        /// let result = Err(e);
        ///
        /// result.add_context().msg("This is what I was doing here");
        /// ```
        msg,
        /// If this is a [`Result::Err`], adds a message from a [`std::fmt::Arguments`]
        /// to the error as context.
        ///
        /// This can be used with [`std::format_args!`] to create a message
        /// from a format string and arguments.
        args,
    }) where E: DiagLike
}

macro_rules! define_raise_binder {
    (
        $(#[$meta:meta])*
        $v:vis struct $name:ident$(<$($ty_var:ident),*>)?(
            $mapper_ty:ty => {
                type Kind = $kind_var:ident,
                Diag => $diag_ty:ty,
                AnyDiag => $any_ty:ty,
                Docs => {
                    $(#[$kind_meta:meta])* kind,
                    $(#[$kind_msg_meta:meta])* kind_msg,
                    $(#[$kind_args_meta:meta])* kind_args,
                    $(#[$msg_meta:meta])* msg,
                    $(#[$args_meta:meta])* args,
                },
            })
        $(where $($where_clause:tt)*)?
    ) => {
        $(#[$meta])*
        $v struct $name$(<$($ty_var),*>)?(InnerBinder<$mapper_ty>);

        impl$(<$($ty_var),*>)? $name$(<$($ty_var),*>)?
            $(where $($where_clause)*)?
        {
            #[track_caller]
            pub(crate) fn new(mapper: $mapper_ty) -> Self {
                Self(InnerBinder::new(mapper))
            }

            $(#[$kind_meta])*
            $v fn kind<$kind_var>(self, kind: $kind_var) -> $diag_ty
            where
                $kind_var: Kind + Reportable,
            {
                self.0.into_diag(move || KindFinding::new_kind(kind))
            }

            $(#[$kind_msg_meta])*
            $v fn kind_msg<$kind_var, M>(self, kind: $kind_var, msg: M) -> $diag_ty
            where
                $kind_var: Kind,
                M: Reportable,
            {
                self.0.into_diag(move || KindFinding::new_kind_msg(kind, msg))
            }

            $(#[$kind_args_meta])*
            $v fn kind_args<$kind_var>(self, kind: $kind_var, args: std::fmt::Arguments<'_>) -> $diag_ty
            where
                $kind_var: Kind,
            {
                self.0.into_diag(move || KindFinding::new_kind_args(kind, args))
            }

            $(#[$msg_meta])*
            $v fn msg<M>(self, msg: M) -> $any_ty
            where
                M: Reportable,
            {
                self.0.into_diag(move || MessageFinding::new_msg(msg))
            }

            $(#[$args_meta])*
            $v fn args(self, args: std::fmt::Arguments<'_>) -> $any_ty {
                self.0.into_diag(move || MessageFinding::new_args(args))
            }
        }
    };
}

define_raise_binder! {
    pub struct RaiseBinder<T>(ValueMapper<T> => {
        type Kind = K,
        Diag => Diag<K>,
        AnyDiag => AnyDiag,
        Docs => {
            kind,
            kind_msg,
            kind_args,
            msg,
            args,
        },
    }) where T: IntoCause
}

define_raise_binder! {
    pub struct OptionRaiseBinder<T>(Option<T> => {
        type Kind = K,
        Diag => Result<T, Diag<K>>,
        AnyDiag => Result<T, AnyDiag>,
        Docs => {
            kind,
            kind_msg,
            kind_args,
            msg,
            args,
        },
    })
}

define_raise_binder! {
    pub struct ResultRaiseBinder<T, E>(Result<T, E> => {
        type Kind = K,
        Diag => Result<T, Diag<K>>,
        AnyDiag => Result<T, AnyDiag>,
        Docs => {
            kind,
            kind_msg,
            kind_args,
            msg,
            args,
        },
    }) where E: IntoCause
}

pub(crate) trait Mapper {
    type In;
    type Out<T>;

    #[doc(hidden)]
    fn map_value<T>(self, func: impl FnOnce(Self::In) -> T) -> Self::Out<T>;
}

pub(crate) struct ValueMapper<T>(T);

impl<T> ValueMapper<T> {
    pub(crate) fn new(value: T) -> Self {
        Self(value)
    }
}

impl<T> Mapper for ValueMapper<T> {
    type In = T;
    type Out<D> = D;

    fn map_value<D>(self, func: impl FnOnce(Self::In) -> D) -> Self::Out<D> {
        func(self.0)
    }
}

impl<V, E> Mapper for Result<V, E> {
    type In = E;
    type Out<D> = Result<V, D>;

    fn map_value<T>(self, func: impl FnOnce(Self::In) -> T) -> Self::Out<T> {
        match self {
            Ok(val) => Ok(val),
            Err(err) => Err(func(err)),
        }
    }
}

impl<T> Mapper for Option<T> {
    type In = NullCause;

    type Out<D> = Result<T, D>;

    fn map_value<D>(self, func: impl FnOnce(Self::In) -> D) -> Self::Out<D> {
        match self {
            Some(val) => Ok(val),
            None => Err(func(NullCause)),
        }
    }
}
