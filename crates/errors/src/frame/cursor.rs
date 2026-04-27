use std::fmt;

use itertools::Either;

use crate::{
    Kind,
    dyn_err_conversion::try_cast_to_any_diag_ref,
    fmt_helpers::{Indent, indent_fmt},
    frame::{Context, Frame, FrameRoot},
    locations::SourceLoc,
};

pub(super) trait CursorIterator: ExactSizeIterator + DoubleEndedIterator {}
impl<I> CursorIterator for I where I: ExactSizeIterator + DoubleEndedIterator {}

fn error_causes_iter<'a>(
    error: &'a (dyn std::error::Error + 'static),
) -> impl CursorIterator<Item = FrameCursor<'a>> + 'a {
    error.source().into_iter().map(FrameCursor::from_std_error)
}

#[derive(Clone, Debug)]
enum CursorRoot<'a> {
    Frame(&'a Frame),
    StdErr(&'a (dyn std::error::Error + 'static)),
}

/// Helper for frames, giving a generalized structure of printable
/// values
#[derive(Clone, Debug)]
pub(super) struct FrameCursor<'a> {
    root: CursorRoot<'a>,
}

impl<'a> FrameCursor<'a> {
    pub(super) fn from_frame(frame: &'a Frame) -> Self {
        Self {
            root: CursorRoot::Frame(frame),
        }
    }

    pub(super) fn from_std_error(err: &'a (dyn std::error::Error + 'static)) -> Self {
        if let Some(any_diag) = try_cast_to_any_diag_ref(err) {
            return Self {
                root: CursorRoot::Frame(any_diag.frame()),
            };
        }
        Self {
            root: CursorRoot::StdErr(err),
        }
    }

    pub(super) fn write_message_to(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self.root {
            CursorRoot::Frame(frame) => {
                write!(fmt, "{}", frame.inner.frame_root.message())
            }
            CursorRoot::StdErr(error) => {
                write!(fmt, "{error}")
            }
        }
    }
    pub(super) fn location(&self) -> Option<&SourceLoc> {
        match self.root {
            CursorRoot::Frame(inner) => Some(inner.location()),
            CursorRoot::StdErr(_) => None,
        }
    }

    pub(super) fn kind_location(&self) -> Option<&SourceLoc> {
        match self.root {
            CursorRoot::Frame(inner) => Some(inner.kind_location()),
            CursorRoot::StdErr(_) => None,
        }
    }

    pub(super) fn additional_contexts(&self) -> &'a [Context] {
        match self.root {
            CursorRoot::Frame(frame) => &frame.inner.additional_contexts,
            CursorRoot::StdErr(_) => &[],
        }
    }

    pub(super) fn causes(&self) -> impl CursorIterator<Item = FrameCursor<'a>> + 'a {
        match self.root {
            CursorRoot::Frame(frame) => {
                // The frame can either contain a diag-based context, or a
                // std-error context.
                match &frame.inner.frame_root {
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

    pub(super) fn try_kind_ref<K>(&self) -> Option<&K>
    where
        K: Kind,
    {
        match self.root {
            CursorRoot::Frame(frame) => frame.inner.frame_root.downcast_ref(),
            CursorRoot::StdErr(_) => None,
        }
    }

    pub(super) fn into_all_frames_iter(self) -> impl Iterator<Item = FrameCursor<'a>> {
        FrameIter::from_root(self)
    }

    pub(super) fn display_fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write_message_to(f)?;

        if f.alternate() {
            // Print as "<primary message> (<context #1>, <context #2>, ...)"

            let additional_contexts = self.additional_contexts();

            if !additional_contexts.is_empty() {
                f.write_str(" (")?;
                let mut first = true;
                for ctxt in additional_contexts {
                    if first {
                        first = false;
                    } else {
                        f.write_str(", ")?;
                    }
                    write!(f, "{}", ctxt.message)?;
                }
                f.write_str(")")?;
            }
        }

        Ok(())
    }

    pub(super) fn debug_fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            return self.report_fmt(f);
        }

        fmt::Debug::fmt(self, f)
    }

    pub(super) fn report_fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_message_to(f)?;
        if let Some(created_at) = self.location() {
            write!(f, "\n  at {created_at}")?;
        }
        let additional_contexts = self.additional_contexts();
        if additional_contexts.len() == 1 {
            let ctxt = &additional_contexts[0];
            write!(
                f,
                "\n  with context: {:#?}",
                Indent::new(&ctxt.message).indent(16)
            )?;
            write!(f, "\n    at {}", ctxt.created_at)?;
        } else if !additional_contexts.is_empty() {
            write!(f, "\n  with contexts:")?;
            for ctxt in additional_contexts {
                write!(f, "\n  - {:#?}", Indent::new(&ctxt.message).indent(4))?;
                write!(f, "\n    at {}", ctxt.created_at)?;
            }
        }

        let mut causes = self.causes();
        if causes.len() == 1 {
            let cause = causes.next().unwrap();
            write!(f, "\n  Caused by:\n    ")?;
            indent_fmt(f, 4, |f| cause.report_fmt(f))?;
        } else if causes.len() > 0 {
            // We are going to number the entries, so we need to know the
            // character width to make everything line up nicely.
            let max_cause_index = causes.len() - 1;
            // This could be more efficient, but we just print the number - 1,
            // and count the chars.
            let cause_char_len = format!("{max_cause_index}").chars().count();
            write!(f, "\n  Causes:")?;
            for (i, cause) in causes.enumerate() {
                write!(f, "\n    {i:>cause_char_len$}: ")?;
                indent_fmt(f, 6 + cause_char_len, |f| cause.report_fmt(f))?;
            }
        }
        Ok(())
    }
}

pub(super) struct FrameIter<'a> {
    frame_stack: Vec<FrameCursor<'a>>,
}

impl<'a> FrameIter<'a> {
    fn from_root(root_cursor: FrameCursor<'a>) -> Self {
        FrameIter {
            frame_stack: vec![root_cursor],
        }
    }
}

impl<'a> Iterator for FrameIter<'a> {
    type Item = FrameCursor<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let next_cursor = self.frame_stack.pop()?;
        self.frame_stack.extend(next_cursor.causes().rev());
        Some(next_cursor)
    }
}
