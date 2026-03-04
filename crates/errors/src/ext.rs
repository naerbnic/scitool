use std::{marker::PhantomData, panic::Location};

use crate::{
    AnyDiag, Diag, DiagLike, Kind, MaybeDiag, Reportable,
    binders::{IntoCause, OptionRaiseBinder, ResultContextBinder, ResultRaiseBinder},
    finding::{KindFinding, MessageFinding},
    frame::Frame,
};

#[must_use]
pub struct RaisedKind<K>
where
    K: Kind,
{
    finding: KindFinding<K>,
    created_at: &'static Location<'static>,
}

impl<K> RaisedKind<K>
where
    K: Kind,
{
    pub fn maybe(self) -> RaisedMaybe<K> {
        self.into()
    }
}

#[must_use]
pub struct RaisedMessage {
    finding: MessageFinding,
    created_at: &'static Location<'static>,
}

impl RaisedMessage {
    pub fn maybe<K>(self) -> RaisedMaybe<K>
    where
        K: Kind,
    {
        self.into()
    }

    pub(crate) fn add_as_context<D>(self, mut diag: D) -> D
    where
        D: DiagLike,
    {
        diag.add_context_message(self);
        diag
    }

    pub(crate) fn add_to_frame_as_context(self, frame: &mut Frame) {
        frame.add_context(self.finding, self.created_at);
    }
}

#[must_use]
pub struct RaisedMaybe<K>
where
    K: Kind,
{
    finding: Result<KindFinding<K>, MessageFinding>,
    created_at: &'static Location<'static>,
}

impl<K> From<RaisedMessage> for RaisedMaybe<K>
where
    K: Kind,
{
    fn from(value: RaisedMessage) -> Self {
        RaisedMaybe {
            finding: Err(value.finding),
            created_at: value.created_at,
        }
    }
}

impl<K> From<RaisedKind<K>> for RaisedMaybe<K>
where
    K: Kind,
{
    fn from(value: RaisedKind<K>) -> Self {
        RaisedMaybe {
            finding: Ok(value.finding),
            created_at: value.created_at,
        }
    }
}

pub trait RaisedToDiag: Sized {
    type Diag;
    fn into_diag(self, causes: impl IntoIterator<Item = impl IntoCause>) -> Self::Diag;

    fn into_new_diag(self) -> Self::Diag {
        self.into_diag(std::iter::empty::<std::convert::Infallible>())
    }
}

impl<K> RaisedToDiag for RaisedKind<K>
where
    K: Kind,
{
    type Diag = Diag<K>;

    fn into_diag(self, causes: impl IntoIterator<Item = impl IntoCause>) -> Self::Diag {
        Diag::from_finding_and_causes(self.finding, causes, self.created_at)
    }
}

impl RaisedToDiag for RaisedMessage {
    type Diag = AnyDiag;

    fn into_diag(self, causes: impl IntoIterator<Item = impl IntoCause>) -> Self::Diag {
        AnyDiag::from_finding_and_causes(self.finding, causes, self.created_at)
    }
}

impl<K> RaisedToDiag for RaisedMaybe<K>
where
    K: Kind,
{
    type Diag = MaybeDiag<K>;

    fn into_diag(self, causes: impl IntoIterator<Item = impl IntoCause>) -> Self::Diag {
        match self.finding {
            Ok(finding) => Diag::from_finding_and_causes(finding, causes, self.created_at).into(),
            Err(finding) => {
                AnyDiag::from_finding_and_causes(finding, causes, self.created_at).into()
            }
        }
    }
}

#[must_use]
pub struct Raiser<'a> {
    // A field to prevent users from creating in in situ.
    created_at: &'static Location<'static>,
    _phantom: PhantomData<&'a ()>,
}

impl Raiser<'_> {
    #[track_caller]
    pub(crate) fn new() -> Self {
        Self {
            created_at: Location::caller(),
            _phantom: PhantomData,
        }
    }

    pub(crate) fn created_at(&self) -> &'static Location<'static> {
        self.created_at
    }

    pub(crate) fn kind_finding<K>(self, finding: KindFinding<K>) -> RaisedKind<K>
    where
        K: Kind,
    {
        RaisedKind {
            finding,
            created_at: self.created_at,
        }
    }

    pub(crate) fn msg_finding(self, finding: MessageFinding) -> RaisedMessage {
        RaisedMessage {
            finding,
            created_at: self.created_at,
        }
    }

    pub fn kind<K>(self, kind: K) -> RaisedKind<K>
    where
        K: Kind + Reportable,
    {
        self.kind_finding(KindFinding::new_kind(kind))
    }

    pub fn kind_msg<K, M>(self, kind: K, msg: M) -> RaisedKind<K>
    where
        K: Kind,
        M: Reportable,
    {
        self.kind_finding(KindFinding::new_kind_msg(kind, msg))
    }

    pub fn kind_args<K>(self, kind: K, args: std::fmt::Arguments<'_>) -> RaisedKind<K>
    where
        K: Kind,
    {
        self.kind_finding(KindFinding::new_kind_args(kind, args))
    }

    pub fn msg<M>(self, msg: M) -> RaisedMessage
    where
        M: Reportable,
    {
        self.msg_finding(MessageFinding::new_msg(msg))
    }

    pub fn args(self, args: std::fmt::Arguments<'_>) -> RaisedMessage {
        self.msg_finding(MessageFinding::new_args(args))
    }
}

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
    fn with_context(self) -> ResultContextBinder<Self::OkT, Self::ErrT>
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
    fn raise(self) -> ResultRaiseBinder<Self::OkT, Self::ErrT>
    where
        Self::ErrT: IntoCause;

    fn raise_with<R>(self, raise_fn: impl FnOnce(Raiser<'_>) -> R) -> Result<Self::OkT, R::Diag>
    where
        Self::ErrT: IntoCause,
        R: RaisedToDiag;

    fn reraise(self) -> Result<Self::OkT, <Self::ErrT as DiagStdError>::Diag>
    where
        Self::ErrT: DiagStdError;

    fn map_raise<R>(
        self,
        raise_fn: impl FnOnce(&Self::ErrT, Raiser<'_>) -> R,
    ) -> Result<Self::OkT, R::Diag>
    where
        Self::ErrT: IntoCause,
        R: RaisedToDiag;
}

impl<T, E> ResultExt for Result<T, E> {
    type OkT = T;
    type ErrT = E;

    #[track_caller]
    fn with_context(self) -> ResultContextBinder<T, E>
    where
        Self::ErrT: DiagLike,
    {
        ResultContextBinder::new(self)
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
    fn raise(self) -> ResultRaiseBinder<T, E>
    where
        Self::ErrT: IntoCause,
    {
        ResultRaiseBinder::new(self)
    }

    #[track_caller]
    fn raise_with<R>(self, raise_fn: impl FnOnce(Raiser<'_>) -> R) -> Result<Self::OkT, R::Diag>
    where
        Self::ErrT: IntoCause,
        R: RaisedToDiag,
    {
        let raiser = Raiser::new();
        self.map_err(|err| raise_fn(raiser).into_diag([err]))
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
}

pub trait OptionExt {
    type Value;
    fn raise(self) -> OptionRaiseBinder<Self::Value>;

    fn raise_with<R>(self, raise_fn: impl FnOnce(Raiser<'_>) -> R) -> Result<Self::Value, R::Diag>
    where
        R: RaisedToDiag;
}

impl<T> OptionExt for Option<T> {
    type Value = T;

    #[track_caller]
    fn raise(self) -> OptionRaiseBinder<T> {
        OptionRaiseBinder::new(self)
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
