use std::fmt;

use itertools::Either;

use crate::{
    dyn_err_conversion::try_cast_to_any_diag_ref,
    frame::{Context, FrameRoot, Inner},
    locations::SourceLoc,
};

enum CursorRoot<'a> {
    Frame(&'a Inner),
    StdErr(&'a (dyn std::error::Error + 'static)),
}

fn error_causes_iter<'a>(
    error: &'a (dyn std::error::Error + 'static),
) -> impl ExactSizeIterator<Item = FrameCursor<'a>> + 'a {
    error.source().into_iter().map(FrameCursor::from_std_error)
}

/// Helper for frames, giving a generalized structure of printable
/// values
pub(super) struct FrameCursor<'a> {
    root: CursorRoot<'a>,
}

impl<'a> FrameCursor<'a> {
    pub(super) fn from_frame(inner: &'a Inner) -> Self {
        Self {
            root: CursorRoot::Frame(inner),
        }
    }

    pub(super) fn from_std_error(err: &'a (dyn std::error::Error + 'static)) -> Self {
        if let Some(any_diag) = try_cast_to_any_diag_ref(err) {
            return Self {
                root: CursorRoot::Frame(&any_diag.frame().inner),
            };
        }
        Self {
            root: CursorRoot::StdErr(err),
        }
    }

    pub(super) fn write_message_to(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self.root {
            CursorRoot::Frame(frame) => {
                write!(fmt, "{}", frame.frame_root.message())
            }
            CursorRoot::StdErr(error) => {
                write!(fmt, "{error}")
            }
        }
    }
    pub(super) fn created_at(&self) -> Option<&SourceLoc> {
        match self.root {
            CursorRoot::Frame(inner) => Some(inner.frame_root.created_at()),
            CursorRoot::StdErr(_) => None,
        }
    }
    pub(super) fn additional_contexts(&self) -> &[Context] {
        match self.root {
            CursorRoot::Frame(inner) => &inner.additional_contexts,
            CursorRoot::StdErr(_) => &[],
        }
    }
    pub(super) fn causes(&self) -> impl ExactSizeIterator<Item = FrameCursor<'a>> + 'a {
        match self.root {
            CursorRoot::Frame(inner) => {
                // The frame can either contain a diag-based context, or a
                // std-error context.
                match &inner.frame_root {
                    FrameRoot::DiagRoot { causes, .. } => {
                        Either::Left(causes[..].iter().map(|f| f.get_cursor()))
                    }
                    FrameRoot::StdErrorRoot { source, .. } => {
                        Either::Right(error_causes_iter(&**source))
                    }
                }
            }
            CursorRoot::StdErr(error) => {
                assert!(try_cast_to_any_diag_ref(error).is_none());
                Either::Right(error_causes_iter(error))
            }
        }
    }
}
