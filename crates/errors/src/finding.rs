use std::marker::PhantomData;

use crate::{Kind, Reportable, reportable::ReportableHandle};

pub(crate) struct KindFinding<K>
where
    K: Kind,
{
    err_like: ReportableHandle,
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
            err_like: ReportableHandle::new(kind),
            _phantom: PhantomData,
        }
    }

    pub(crate) fn new_kind_msg<M>(kind: K, msg: M) -> Self
    where
        M: Reportable,
    {
        Self {
            err_like: ReportableHandle::from_split(kind, msg),
            _phantom: PhantomData,
        }
    }

    pub(crate) fn new_kind_args(kind: K, args: std::fmt::Arguments<'_>) -> Self {
        let err_like = if let Some(static_str) = args.as_str() {
            ReportableHandle::from_split(kind, static_str)
        } else {
            ReportableHandle::from_split(kind, args.to_string())
        };
        Self {
            err_like,
            _phantom: PhantomData,
        }
    }

    #[must_use]
    pub(crate) fn into_handle(self) -> ReportableHandle {
        self.err_like
    }

    pub(crate) fn append_reportable(self, rep: impl Reportable) -> Self {
        Self {
            err_like: self.err_like.append_reportable(rep),
            _phantom: PhantomData,
        }
    }
}

pub(crate) struct MessageFinding {
    err_like: ReportableHandle,
}

impl MessageFinding {
    pub(crate) fn new_msg<M>(msg: M) -> Self
    where
        M: Reportable,
    {
        Self {
            err_like: ReportableHandle::from_report_only(msg),
        }
    }

    pub(crate) fn new_args(args: std::fmt::Arguments<'_>) -> Self {
        let err_like = if let Some(static_str) = args.as_str() {
            ReportableHandle::from_report_only(static_str)
        } else {
            ReportableHandle::from_report_only(args.to_string())
        };
        Self { err_like }
    }

    #[must_use]
    pub(crate) fn into_err_like(self) -> ReportableHandle {
        self.err_like
    }

    pub(crate) fn append_reportable(self, rep: impl Reportable) -> Self {
        Self {
            err_like: self.err_like.append_reportable(rep),
        }
    }
}
