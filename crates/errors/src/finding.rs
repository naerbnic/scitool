use std::marker::PhantomData;

use crate::{Kind, Reportable, reportable::ReportableHandle};

pub(crate) struct KindFinding<K>
where
    K: Kind,
{
    rep_handle: ReportableHandle,
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
            rep_handle: ReportableHandle::new(kind),
            _phantom: PhantomData,
        }
    }

    pub(crate) fn new_kind_msg<M>(kind: K, msg: M) -> Self
    where
        M: Reportable,
    {
        Self {
            rep_handle: ReportableHandle::from_split(kind, msg),
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
            rep_handle: err_like,
            _phantom: PhantomData,
        }
    }

    #[must_use]
    pub(crate) fn into_handle(self) -> ReportableHandle {
        self.rep_handle
    }

    pub(crate) fn append_reportable(self, rep: impl Reportable) -> Self {
        Self {
            rep_handle: self.rep_handle.append_reportable(rep),
            _phantom: PhantomData,
        }
    }
}

pub(crate) struct MessageFinding {
    rep_handle: ReportableHandle,
}

impl MessageFinding {
    pub(crate) fn new_msg<M>(msg: M) -> Self
    where
        M: Reportable,
    {
        Self {
            rep_handle: ReportableHandle::from_report_only(msg),
        }
    }

    pub(crate) fn new_args(args: std::fmt::Arguments<'_>) -> Self {
        let err_like = if let Some(static_str) = args.as_str() {
            ReportableHandle::from_report_only(static_str)
        } else {
            ReportableHandle::from_report_only(args.to_string())
        };
        Self {
            rep_handle: err_like,
        }
    }

    #[must_use]
    pub(crate) fn into_err_like(self) -> ReportableHandle {
        self.rep_handle
    }

    pub(crate) fn append_reportable(self, rep: impl Reportable) -> Self {
        Self {
            rep_handle: self.rep_handle.append_reportable(rep),
        }
    }
}
