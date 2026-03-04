use crate::{
    ContextBind, ContextBinder, DiagLike, RaisedMessage, Raiser,
    binders::{
        Bind, ErrResultBind, IntoCause, OptionBind, RaiseBinder, ResultBind, ResultContextBind,
    },
    out,
    raiser::RaisedToDiag,
};

/// A trait that marks specific `std::error::Error` types that can be converted
/// back to Diags if requested.
pub trait DiagStdError: std::error::Error + Send + Sync + 'static {
    type Diag: DiagLike;

    fn into_diag(self) -> Self::Diag;
}

pub trait ResultExt: Sized {
    type OkT;
    type ErrT;

    /// Creates a binder used to add context to an error.
    fn with_context(self) -> ContextBinder<impl ContextBind<Out = Result<Self::OkT, Self::ErrT>>>
    where
        Self::ErrT: DiagLike;

    /// Adds context to an error, if any, allowing the message to be derived
    /// from the error value itself.
    fn map_with_context(
        self,
        func: impl FnOnce(&Self::ErrT, Raiser<'_>) -> RaisedMessage,
    ) -> Result<Self::OkT, Self::ErrT>
    where
        Self::ErrT: DiagLike;

    /// Used to raise a new `DiagLike` error from this result.
    ///
    /// This is part of a fluent API. Calling a method on the returned object
    /// will return the new Result with the new error.
    fn raise(self) -> RaiseBinder<impl Bind<Out = out::Result<Self::OkT>>>
    where
        Self::ErrT: IntoCause;

    fn raise_err(self) -> RaiseBinder<impl Bind<Out = out::Result<Self::OkT>>>
    where
        Self::ErrT: std::error::Error + Send + Sync + 'static;

    fn raise_with<R>(self, raise_fn: impl FnOnce(Raiser<'_>) -> R) -> Result<Self::OkT, R::Diag>
    where
        Self::ErrT: IntoCause,
        R: RaisedToDiag;

    fn reraise(self) -> Result<Self::OkT, <Self::ErrT as DiagStdError>::Diag>
    where
        Self::ErrT: DiagStdError;

    fn raise_err_with<R>(
        self,
        raise_fn: impl FnOnce(Raiser<'_>) -> R,
    ) -> Result<Self::OkT, R::Diag>
    where
        Self::ErrT: std::error::Error + Send + Sync + 'static,
        R: RaisedToDiag;

    fn map_raise<R>(
        self,
        raise_fn: impl FnOnce(&Self::ErrT, Raiser<'_>) -> R,
    ) -> Result<Self::OkT, R::Diag>
    where
        Self::ErrT: IntoCause,
        R: RaisedToDiag;

    fn map_raise_err<R>(
        self,
        func: impl FnOnce(&Self::ErrT, Raiser<'_>) -> R,
    ) -> Result<Self::OkT, R::Diag>
    where
        Self::ErrT: std::error::Error + Send + Sync + 'static,
        R: RaisedToDiag;
}

impl<T, E> ResultExt for Result<T, E> {
    type OkT = T;
    type ErrT = E;

    #[track_caller]
    fn with_context(self) -> ContextBinder<impl ContextBind<Out = Result<Self::OkT, Self::ErrT>>>
    where
        Self::ErrT: DiagLike,
    {
        ContextBinder::new(ResultContextBind::new(self))
    }

    fn map_with_context(
        self,
        func: impl FnOnce(&Self::ErrT, Raiser<'_>) -> RaisedMessage,
    ) -> Result<Self::OkT, Self::ErrT>
    where
        Self::ErrT: DiagLike,
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
        Self::ErrT: IntoCause,
    {
        RaiseBinder::new(ResultBind::new(self))
    }

    #[track_caller]
    fn raise_err(self) -> RaiseBinder<impl Bind<Out = out::Result<T>>>
    where
        Self::ErrT: std::error::Error + Send + Sync + 'static,
    {
        RaiseBinder::new(ErrResultBind::new(self))
    }

    #[track_caller]
    fn raise_with<R>(self, raise_fn: impl FnOnce(Raiser<'_>) -> R) -> Result<Self::OkT, R::Diag>
    where
        Self::ErrT: IntoCause,
        R: RaisedToDiag,
    {
        self.map_raise(|_, r| raise_fn(r))
    }

    #[track_caller]
    fn raise_err_with<R>(self, raise_fn: impl FnOnce(Raiser<'_>) -> R) -> Result<Self::OkT, R::Diag>
    where
        Self::ErrT: std::error::Error + Send + Sync + 'static,
        R: RaisedToDiag,
    {
        self.map_raise_err(|_, r| raise_fn(r))
    }

    fn reraise(self) -> Result<Self::OkT, <Self::ErrT as DiagStdError>::Diag>
    where
        Self::ErrT: DiagStdError,
    {
        self.map_err(DiagStdError::into_diag)
    }

    #[track_caller]
    fn map_raise<R>(
        self,
        raise_fn: impl FnOnce(&Self::ErrT, Raiser<'_>) -> R,
    ) -> Result<Self::OkT, R::Diag>
    where
        Self::ErrT: IntoCause,
        R: RaisedToDiag,
    {
        let raiser = Raiser::new();
        self.map_err(|err| raise_fn(&err, raiser).into_diag([err]))
    }
    #[track_caller]
    fn map_raise_err<R>(
        self,
        func: impl FnOnce(&Self::ErrT, Raiser<'_>) -> R,
    ) -> Result<Self::OkT, R::Diag>
    where
        Self::ErrT: std::error::Error + Send + Sync + 'static,
        R: RaisedToDiag,
    {
        let raiser = Raiser::new();
        self.map_err(|err| func(&err, raiser).into_diag_with_appended(err))
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
