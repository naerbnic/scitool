//! Types for managing the causes of diags.

use crate::{frame::Frame, locations::SourceLoc, reportable::WeakReportableHandle};

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
    fn into_cause(self, created_at: SourceLoc) -> Cause {
        Cause::from_frame(Frame::from_box_std_error(Box::new(self), created_at))
    }
}

impl IntoCause for Cause {
    fn into_cause(self, _created_at: SourceLoc) -> Cause {
        self
    }
}
