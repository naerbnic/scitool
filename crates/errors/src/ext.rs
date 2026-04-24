use crate::{
    ContextBinder, DiagLike, RaisedMessage, Raiser,
    binders::{
        Bind, ContextBind, ErrResultBind, IntoCause, OptionBind, RaiseBinder, ResultBind,
        ResultContextBind,
    },
    out,
    raiser::RaisedToDiag,
};

/// An extension trait for [`Result<T, E>`], providing different kinds of error
/// dispatching depending on the bounds of `E`
///
/// Methods are intended to be fluent
pub trait ResultExt: Sized {
    /// The value type for the result (i.e. the `T` of [`Result<T, E>`])
    type Value;
    /// The error type for the result (i.e. the `E` of [`Result<T, E>`])
    type Error;

    /// Creates a binder used to add context to a [`DiagLike`] error, if this
    /// is a [`Result::Err`] variant.
    fn with_context(
        self,
    ) -> ContextBinder<impl ContextBind<Out = Result<Self::Value, Self::Error>>>
    where
        Self::Error: DiagLike;

    /// Adds context to an error, if any, allowing the message to be derived
    /// from the error value itself.
    fn map_with_context(
        self,
        func: impl FnOnce(&Self::Error, Raiser<'_>) -> RaisedMessage,
    ) -> Result<Self::Value, Self::Error>
    where
        Self::Error: DiagLike;

    /// Used to raise a new `DiagLike` error from this result.
    ///
    /// This is part of a fluent API. Calling a method on the returned object
    /// will return the new Result with the new error.
    fn raise(self) -> RaiseBinder<impl Bind<Out = out::Result<Self::Value>>>
    where
        Self::Error: IntoCause;

    fn raise_err(self) -> RaiseBinder<impl Bind<Out = out::Result<Self::Value>>>
    where
        Self::Error: std::error::Error + Send + Sync + 'static;

    fn raise_with<R>(self, raise_fn: impl FnOnce(Raiser<'_>) -> R) -> Result<Self::Value, R::Diag>
    where
        Self::Error: IntoCause,
        R: RaisedToDiag;

    fn raise_err_with<R>(
        self,
        raise_fn: impl FnOnce(Raiser<'_>) -> R,
    ) -> Result<Self::Value, R::Diag>
    where
        Self::Error: std::error::Error + Send + Sync + 'static,
        R: RaisedToDiag;

    fn map_raise<R>(
        self,
        raise_fn: impl FnOnce(&Self::Error, Raiser<'_>) -> R,
    ) -> Result<Self::Value, R::Diag>
    where
        Self::Error: IntoCause,
        R: RaisedToDiag;

    fn map_raise_err<R>(
        self,
        func: impl FnOnce(&Self::Error, Raiser<'_>) -> R,
    ) -> Result<Self::Value, R::Diag>
    where
        Self::Error: std::error::Error + Send + Sync + 'static,
        R: RaisedToDiag;
}

impl<T, E> ResultExt for Result<T, E> {
    type Value = T;
    type Error = E;

    #[track_caller]
    fn with_context(self) -> ContextBinder<impl ContextBind<Out = Result<Self::Value, Self::Error>>>
    where
        Self::Error: DiagLike,
    {
        ContextBinder::new(ResultContextBind::new(self))
    }

    fn map_with_context(
        self,
        func: impl FnOnce(&Self::Error, Raiser<'_>) -> RaisedMessage,
    ) -> Result<Self::Value, Self::Error>
    where
        Self::Error: DiagLike,
    {
        let raiser = Raiser::new();
        match self {
            Ok(ok) => Ok(ok),
            Err(err) => Err(func(&err, raiser).add_as_context(err)),
        }
    }

    #[track_caller]
    fn raise(self) -> RaiseBinder<impl Bind<Out = out::Result<T>>>
    where
        Self::Error: IntoCause,
    {
        RaiseBinder::new(ResultBind::new(self))
    }

    #[track_caller]
    fn raise_err(self) -> RaiseBinder<impl Bind<Out = out::Result<T>>>
    where
        Self::Error: std::error::Error + Send + Sync + 'static,
    {
        RaiseBinder::new(ErrResultBind::new(self))
    }

    #[track_caller]
    fn raise_with<R>(self, raise_fn: impl FnOnce(Raiser<'_>) -> R) -> Result<Self::Value, R::Diag>
    where
        Self::Error: IntoCause,
        R: RaisedToDiag,
    {
        self.map_raise(|_, r| raise_fn(r))
    }

    #[track_caller]
    fn raise_err_with<R>(
        self,
        raise_fn: impl FnOnce(Raiser<'_>) -> R,
    ) -> Result<Self::Value, R::Diag>
    where
        Self::Error: std::error::Error + Send + Sync + 'static,
        R: RaisedToDiag,
    {
        self.map_raise_err(|_, r| raise_fn(r))
    }

    #[track_caller]
    fn map_raise<R>(
        self,
        raise_fn: impl FnOnce(&Self::Error, Raiser<'_>) -> R,
    ) -> Result<Self::Value, R::Diag>
    where
        Self::Error: IntoCause,
        R: RaisedToDiag,
    {
        let raiser = Raiser::new();
        self.map_err(|err| raise_fn(&err, raiser).into_diag([err]))
    }
    #[track_caller]
    fn map_raise_err<R>(
        self,
        func: impl FnOnce(&Self::Error, Raiser<'_>) -> R,
    ) -> Result<Self::Value, R::Diag>
    where
        Self::Error: std::error::Error + Send + Sync + 'static,
        R: RaisedToDiag,
    {
        let raiser = Raiser::new();
        self.map_err(|err| func(&err, raiser).into_diag_with_err(err))
    }
}

pub trait OptionExt {
    type Value;
    fn raise(self) -> RaiseBinder<impl Bind<Out = out::Result<Self::Value>>>;

    fn raise_with<R>(self, raise_fn: impl FnOnce(Raiser<'_>) -> R) -> Result<Self::Value, R::Diag>
    where
        R: RaisedToDiag;
}

impl<T> OptionExt for Option<T> {
    type Value = T;

    #[track_caller]
    fn raise(self) -> RaiseBinder<impl Bind<Out = out::Result<Self::Value>>> {
        RaiseBinder::new(OptionBind::new(self))
    }

    fn raise_with<R>(self, raise_fn: impl FnOnce(Raiser<'_>) -> R) -> Result<T, R::Diag>
    where
        R: RaisedToDiag,
    {
        self.ok_or_else(|| {
            let raiser = Raiser::new();
            raise_fn(raiser).into_new_diag()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{Diag, Kind};

    use super::*;

    #[derive(Debug, thiserror::Error)]
    #[error("This is a test {0}")]
    struct TestKind(u32);

    impl Kind for TestKind {}

    #[test]
    fn test_result_map_raising_ok() {
        let result: Result<u32, Diag<TestKind>> = Ok(5);
        let raised = result.map_raise(|err, r| r.kind(TestKind(err.kind().0 * 2)));
        assert!(raised.is_ok());
    }

    #[test]
    fn test_result_map_raising_err() {
        let result: Result<u32, Diag<TestKind>> = Err(Diag::new().kind(TestKind(5)));
        let raised = result.map_raise(|err, r| r.kind(TestKind(err.kind().0 * 2)));
        assert_eq!(raised.unwrap_err().kind().0, 10);
    }
}
