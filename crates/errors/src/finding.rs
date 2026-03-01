use std::marker::PhantomData;

use crate::{
    Kind, RaisedKind, RaisedMessage, Raiser, Reportable, ext::RaisedToDiag,
    reportable::BoxedErrLike,
};

pub(crate) struct KindFinding<K>
where
    K: Kind,
{
    err_like: BoxedErrLike,
    _phantom: PhantomData<K>,
}

impl<K> KindFinding<K>
where
    K: Kind,
{
    pub(crate) fn new_kind(kind: K) -> Self
    where
        K: Reportable,
    {
        Self {
            err_like: BoxedErrLike::new(kind),
            _phantom: PhantomData,
        }
    }

    pub(crate) fn new_kind_msg<M>(kind: K, msg: M) -> Self
    where
        M: Reportable,
    {
        Self {
            err_like: BoxedErrLike::from_split(kind, msg),
            _phantom: PhantomData,
        }
    }

    pub(crate) fn new_kind_args(kind: K, args: std::fmt::Arguments<'_>) -> Self {
        let err_like = if let Some(static_str) = args.as_str() {
            BoxedErrLike::from_split(kind, static_str)
        } else {
            BoxedErrLike::from_split(kind, args.to_string())
        };
        Self {
            err_like,
            _phantom: PhantomData,
        }
    }

    #[must_use]
    pub(crate) fn into_err_like(self) -> BoxedErrLike {
        self.err_like
    }
}

pub(crate) struct MessageFinding {
    err_like: BoxedErrLike,
}

impl MessageFinding {
    pub(crate) fn new_msg<M>(msg: M) -> Self
    where
        M: Reportable,
    {
        Self {
            err_like: BoxedErrLike::from_report_only(msg),
        }
    }

    pub(crate) fn new_args(args: std::fmt::Arguments<'_>) -> Self {
        let err_like = if let Some(static_str) = args.as_str() {
            BoxedErrLike::from_report_only(static_str)
        } else {
            BoxedErrLike::from_report_only(args.to_string())
        };
        Self { err_like }
    }

    #[must_use]
    pub(crate) fn into_err_like(self) -> BoxedErrLike {
        self.err_like
    }
}

pub(crate) trait FindingToRaised {
    type Raised: RaisedToDiag;

    fn into_raised(self, raiser: Raiser<'_>) -> Self::Raised;
}

impl<K> FindingToRaised for KindFinding<K>
where
    K: Kind,
{
    type Raised = RaisedKind<K>;

    fn into_raised(self, raiser: Raiser<'_>) -> Self::Raised {
        raiser.kind_finding(self)
    }
}

impl FindingToRaised for MessageFinding {
    type Raised = RaisedMessage;

    fn into_raised(self, raiser: Raiser<'_>) -> Self::Raised {
        raiser.msg_finding(self)
    }
}
