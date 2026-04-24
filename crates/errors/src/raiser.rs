use std::{marker::PhantomData, panic::Location};

use crate::{
    AnyDiag, Diag, DiagLike, Kind, MaybeDiag, Reportable,
    binders::IntoCause,
    dyn_err_conversion::try_convert_to_any_diag,
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
    pub(crate) fn from_finding(
        finding: KindFinding<K>,
        created_at: &'static Location<'static>,
    ) -> Self {
        Self {
            finding,
            created_at,
        }
    }

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
    pub(crate) fn from_finding(
        finding: MessageFinding,
        created_at: &'static Location<'static>,
    ) -> Self {
        Self {
            finding,
            created_at,
        }
    }
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

    fn into_diag_with_appended(self, cause: impl IntoCause) -> Self::Diag;

    fn into_new_diag(self) -> Self::Diag {
        self.into_diag(std::iter::empty::<std::convert::Infallible>())
    }

    fn into_diag_with_err<E>(self, err: E) -> Self::Diag
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        match try_convert_to_any_diag(err) {
            Ok(any_diag) => self.into_diag([any_diag]),
            Err(err) => self.into_diag_with_appended(err),
        }
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

    fn into_diag_with_appended(self, cause: impl IntoCause) -> Self::Diag {
        Diag::from_finding_with_appended_cause(self.finding, cause, self.created_at)
    }
}

impl RaisedToDiag for RaisedMessage {
    type Diag = AnyDiag;

    fn into_diag(self, causes: impl IntoIterator<Item = impl IntoCause>) -> Self::Diag {
        AnyDiag::from_finding_and_causes(self.finding, causes, self.created_at)
    }

    fn into_diag_with_appended(self, cause: impl IntoCause) -> Self::Diag {
        AnyDiag::from_finding_with_appended_cause(self.finding, cause, self.created_at)
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

    fn into_diag_with_appended(self, cause: impl IntoCause) -> Self::Diag {
        match self.finding {
            Ok(finding) => {
                Diag::from_finding_with_appended_cause(finding, cause, self.created_at).into()
            }
            Err(finding) => {
                AnyDiag::from_finding_with_appended_cause(finding, cause, self.created_at).into()
            }
        }
    }
}

/// A handle passed to [`DiagLike`]-generating functions to pass on a kind or
/// printable message.
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

    pub(crate) fn kind_finding<K>(self, finding: KindFinding<K>) -> RaisedKind<K>
    where
        K: Kind,
    {
        RaisedKind::from_finding(finding, self.created_at)
    }

    pub(crate) fn msg_finding(self, finding: MessageFinding) -> RaisedMessage {
        RaisedMessage::from_finding(finding, self.created_at)
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
