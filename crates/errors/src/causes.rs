//! Types for managing the causes of diags.

use crate::{
    Diag, Kind, finding::KindFinding, frame::Frame, locations::SourceLoc,
    reportable::WeakReportableHandle,
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

    pub(crate) fn msg_clone_weak(&self) -> WeakReportableHandle {
        self.0.clone_msg_weak()
    }
}

pub trait IntoCause: Sized {
    #[doc(hidden)]
    fn into_cause(self, created_at: SourceLoc) -> Cause;
}

impl<T> IntoCause for T
where
    T: std::error::Error + Send + Sync + 'static,
{
    #[track_caller]
    fn into_cause(self, created_at: SourceLoc) -> Cause {
        let cause = Diag::from_finding_and_frames(
            KindFinding::new_kind(StdErrorCause { error: self }),
            vec![],
            created_at.clone(),
        );
        cause.into_cause(created_at)
    }
}

impl IntoCause for Cause {
    fn into_cause(self, _created_at: SourceLoc) -> Cause {
        self
    }
}
