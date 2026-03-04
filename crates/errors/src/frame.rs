use std::{
    error::Error as StdError,
    fmt::{self, Debug, Display},
    marker::PhantomData,
    panic::Location,
};

use crate::{
    Kind,
    finding::MessageFinding,
    fmt_helpers::Indent,
    reportable::{Reportable, ReportableHandle, WeakReportableHandle},
};

#[derive(Debug)]
struct Context {
    created_at: &'static Location<'static>,
    message: ReportableHandle,
}

/// Helper for frames, giving a generalized structure of printable
/// values
struct FrameFormatContext<'a> {
    base_context: &'a Context,
    additional_contexts: &'a [Context],
    causes: &'a [Frame],
}

struct FrameFormatWrapper<'a>(&'a Frame);

impl Display for FrameFormatWrapper<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.display_fmt(f)
    }
}

impl Debug for FrameFormatWrapper<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.debug_fmt(f)
    }
}

#[derive(Debug)]
struct Inner {
    /// Contexts on top of the given error. In reverse order of creation.
    additional_contexts: Vec<Context>,

    /// The payload of this frame. This may or may not contain a value that
    /// can be queried, but it will be reportable regardless.
    base_context: Context,

    /// The children of this Frame, if any.
    causes: Vec<Frame>,
}

#[derive(Debug)]
pub(crate) struct Frame {
    inner: Box<Inner>,
}

impl Frame {
    pub(crate) fn new(
        root_err: ReportableHandle,
        created_at: &'static Location<'static>,
        causes: Vec<Frame>,
    ) -> Self {
        let code_context = Context {
            created_at,
            message: root_err,
        };
        Self {
            inner: Box::new(Inner {
                additional_contexts: Vec::new(),
                base_context: code_context,
                causes,
            }),
        }
    }

    pub(crate) fn clone_msg_weak(&self) -> WeakReportableHandle {
        self.inner.base_context.message.clone_weak()
    }

    pub(crate) fn add_context(
        &mut self,
        msg: MessageFinding,
        created_at: &'static Location<'static>,
    ) {
        self.inner.additional_contexts.push(Context {
            created_at,
            message: msg.into_err_like(),
        });
    }

    pub(crate) fn has_error<E>(&self) -> bool
    where
        E: Kind,
    {
        self.inner
            .base_context
            .message
            .downcast_ref::<E>()
            .is_some()
    }

    pub(crate) fn try_error_ref<E>(&self) -> Option<&E>
    where
        E: Kind,
    {
        self.inner.base_context.message.downcast_ref()
    }

    pub(crate) fn try_extract_error<E>(&mut self) -> Option<E>
    where
        E: Kind,
    {
        let placeholder = ReportableHandle::from_report_only("<extracted>");
        let error =
            std::mem::replace(&mut self.inner.base_context.message, placeholder).downcast::<E>()?;
        Some(error)
    }

    fn get_format_context(&self) -> FrameFormatContext<'_> {
        FrameFormatContext {
            base_context: &self.inner.base_context,
            additional_contexts: &self.inner.additional_contexts[..],
            causes: &self.inner.causes,
        }
    }

    pub(crate) fn report_fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let fmt_ctxt = self.get_format_context();
        fmt.write_fmt(format_args!("{:#?}", fmt_ctxt.base_context.message))?;
        fmt.write_fmt(format_args!("\n  at {}", fmt_ctxt.base_context.created_at))?;
        if fmt_ctxt.additional_contexts.len() == 1 {
            let ctxt = &fmt_ctxt.additional_contexts[0];
            fmt.write_fmt(format_args!(
                "\n  with context: {:#?}",
                Indent::new(&ctxt.message).indent(16)
            ))?;
            fmt.write_fmt(format_args!("\n    at {}", ctxt.created_at))?;
        } else if !fmt_ctxt.additional_contexts.is_empty() {
            fmt.write_str("\n  with contexts:")?;
            for ctxt in fmt_ctxt.additional_contexts {
                fmt.write_fmt(format_args!(
                    "\n  - {:#?}",
                    Indent::new(&ctxt.message).indent(4)
                ))?;
                fmt.write_fmt(format_args!("\n    at {}", ctxt.created_at))?;
            }
        }

        if fmt_ctxt.causes.len() == 1 {
            let cause = &fmt_ctxt.causes[0];
            fmt.write_fmt(format_args!(
                "\n  Caused by:\n    {:#?}",
                Indent::new(&FrameFormatWrapper(cause)).indent(4)
            ))?;
        } else if !fmt_ctxt.causes.is_empty() {
            // We are going to number the entries, so we need to know the
            // character width to make everything line up nicely.
            let max_cause_index = fmt_ctxt.causes.len() - 1;
            // This could be more efficient, but we just print the number - 1,
            // and count the chars.
            let cause_char_len = format!("{max_cause_index}").chars().count();
            fmt.write_str("\n  Causes:")?;
            for (i, cause) in fmt_ctxt.causes.iter().enumerate() {
                fmt.write_fmt(format_args!(
                    "\n    {i:>cause_char_len$}: {:#?}",
                    Indent::new(&FrameFormatWrapper(cause)).indent(6 + cause_char_len)
                ))?;
            }
        }
        Ok(())
    }

    pub(crate) fn debug_fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        if fmt.alternate() {
            return self.report_fmt(fmt);
        }

        Debug::fmt(self, fmt)
    }

    pub(crate) fn display_fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let fmt_ctxt = self.get_format_context();

        if fmt.alternate() {
            // Print as "<primary message> (<context #1>, <context #2>, ...)"
            fmt.write_fmt(format_args!("{}", fmt_ctxt.base_context.message))?;

            if !fmt_ctxt.additional_contexts.is_empty() {
                fmt.write_str(" (")?;
                let mut first = true;
                for ctxt in fmt_ctxt.additional_contexts {
                    if first {
                        first = false;
                    } else {
                        fmt.write_str(", ")?;
                    }

                    fmt.write_fmt(format_args!("{}", ctxt.message))?;
                }
                fmt.write_str(")")?;
            }
            Ok(())
        } else {
            fmt.write_fmt(format_args!("{}", fmt_ctxt.base_context.message))
        }
    }

    pub(crate) fn causes(&self) -> &[Frame] {
        &self.inner.causes
    }

    pub(crate) fn location(&self) -> &'static Location<'static> {
        if let Some(context) = self.inner.additional_contexts.last() {
            context.created_at
        } else {
            self.inner.base_context.created_at
        }
    }

    /// Returns the root source of this frame's error. If this is a
    /// context-based frame, then this is the location of the originating
    /// context.
    pub(crate) fn err_location(&self) -> &'static Location<'static> {
        self.inner.base_context.created_at
    }

    #[expect(unsafe_code, reason = "For casts between transparent types.")]
    pub(crate) fn as_dyn_error(&self) -> &(dyn StdError + Send + Sync + 'static) {
        // SAFETY: This cast is between a type, and another type that is
        // `repr(transparent)` to the other, which is considered safe.
        let wrapped: &FrameErrorWrapper =
            unsafe { &*std::ptr::from_ref(self).cast::<FrameErrorWrapper>() };
        wrapped
    }

    pub(crate) fn view(&self) -> ErrorView<'_> {
        ErrorView { error: self }
    }

    pub(crate) fn all_frames(&self) -> impl Iterator<Item = ErrorView<'_>> {
        FrameIter::from_frame_slice(std::slice::from_ref(self)).map(Frame::view)
    }

    pub(crate) fn find_errors<E2>(&self) -> impl Iterator<Item = TypedErrorView<'_, E2>>
    where
        E2: Kind,
    {
        self.all_frames().filter_map(|view| view.as_typed::<E2>())
    }

    /// Returns an iterator over all causes of this frame, including the cause
    /// represented by the current [`Frame`]
    pub(crate) fn all_causes(&self) -> impl Iterator<Item = ErrorView<'_>> {
        FrameIter::from_frame_slice(&self.inner.causes).map(Frame::view)
    }
}

pub(crate) struct FrameIter<'a> {
    frame_stack: Vec<&'a [Frame]>,
}

impl<'a> FrameIter<'a> {
    fn from_frame_slice(frames: &'a [Frame]) -> Self {
        FrameIter {
            frame_stack: vec![frames],
        }
    }
}

impl<'a> Iterator for FrameIter<'a> {
    type Item = &'a Frame;

    fn next(&mut self) -> Option<Self::Item> {
        let next_frame = loop {
            let top = self.frame_stack.pop()?;

            if let Some((next_frame, rest)) = top.split_first() {
                self.frame_stack.push(rest);
                break next_frame;
            }
        };

        self.frame_stack.push(next_frame.causes());

        Some(next_frame)
    }
}

/// A private transparent struct, which is able to cast a `&Frame` into a
/// `&dyn StdError`.
#[repr(transparent)]
struct FrameErrorWrapper(Frame);

impl Debug for FrameErrorWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.debug_fmt(f)
    }
}

impl Display for FrameErrorWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.display_fmt(f)
    }
}

impl StdError for FrameErrorWrapper {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        let children = self.0.causes();
        if children.is_empty() {
            // No source error to provide.
            return None;
        }

        if children.len() > 1 {
            // Ambiguous source error.
            return None;
        }

        Some(children[0].as_dyn_error())
    }
}

/// A view of a message context, where a message was added to a Diag trace.
pub struct ContextView<'a> {
    context: &'a Context,
}

impl ContextView<'_> {
    /// The message that was added.
    ///
    /// This will display/debug identically to the message that was added.
    #[must_use]
    pub fn message(&self) -> &dyn Reportable {
        self.context.message.as_reportable()
    }

    /// The code location that the message was created and/or added at.
    #[must_use]
    pub fn location(&self) -> &'static Location<'static> {
        self.context.created_at
    }
}

/// A view on an element of an diag that is known to be of a specific error type.
pub struct TypedErrorView<'a, E>
where
    E: Kind,
{
    // Precondition: error is a frame with a payload of type E.
    error: &'a Frame,
    _phantom: std::marker::PhantomData<E>,
}

impl<'a, E> TypedErrorView<'a, E>
where
    E: Kind,
{
    /// Returns the contained error-like value.
    #[must_use]
    pub fn error(&self) -> &E {
        self.error.try_error_ref().unwrap()
    }

    /// Returns the last location that context was added.
    #[must_use]
    pub fn location(&self) -> &'static Location<'static> {
        self.error.location()
    }

    /// Returns the initial location that this error was raised at.
    #[must_use]
    pub fn err_location(&self) -> &'static Location<'static> {
        self.error.err_location()
    }

    /// Returns an iterator over the contexts that were added to this error.
    pub fn contexts(&self) -> impl Iterator<Item = ContextView<'a>> + 'a {
        self.error
            .inner
            .additional_contexts
            .iter()
            .map(|context| ContextView { context })
    }

    /// An iterator over the immediate children of this error.
    pub fn causes(&self) -> impl Iterator<Item = ErrorView<'a>> + 'a {
        self.error
            .causes()
            .iter()
            .map(|frame| ErrorView { error: frame })
    }

    /// Finds all err-like values in the cause tree of the given type.
    ///
    /// This is most useful for gathering debugging information.
    pub fn find_errors<E2>(&self) -> impl Iterator<Item = TypedErrorView<'a, E2>> + 'a
    where
        E2: Kind,
    {
        self.error.find_errors::<E2>()
    }

    /// Returns an iterator to all the causes of this error, not including the
    /// current error.
    pub fn all_causes<E2>(&self) -> impl Iterator<Item = ErrorView<'a>> + 'a {
        self.error.all_causes()
    }
}

impl<E> fmt::Display for TypedErrorView<'_, E>
where
    E: Kind,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.display_fmt(f)
    }
}

impl<E> fmt::Debug for TypedErrorView<'_, E>
where
    E: Kind,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.debug_fmt(f)
    }
}

/// Returns a view on a raised error whose type is not known.
pub struct ErrorView<'a> {
    error: &'a Frame,
}

impl<'a> ErrorView<'a> {
    /// Returns `Some(err)` for the contained err-like, if the err-like is of
    /// type `E`.
    #[must_use]
    pub fn as_typed<E>(&self) -> Option<TypedErrorView<'a, E>>
    where
        E: Kind,
    {
        if self.error.has_error::<E>() {
            Some(TypedErrorView {
                error: self.error,
                _phantom: PhantomData,
            })
        } else {
            None
        }
    }

    /// Returns true if the err-like is of type `E`.
    #[must_use]
    pub fn has_error_type<E>(&self) -> bool
    where
        E: Kind,
    {
        self.error.has_error::<E>()
    }

    /// Returns the last code location that the error had context added to.
    #[must_use]
    pub fn location(&self) -> &'static Location<'static> {
        self.error.location()
    }

    /// Returns the initial location that this error was raised at.
    #[must_use]
    pub fn err_location(&self) -> &'static Location<'static> {
        self.error.err_location()
    }

    /// Returns a [`std::error::Error`] reference that displays and debugs
    /// as this does, and provides sources down the causal tree.
    ///
    /// Note that casting this error will not allow you to acquire the
    /// core error through this interface. Use the other methods on View instead
    /// if that is desired.
    #[must_use]
    pub fn as_dyn_error(&self) -> &'a (dyn StdError + Send + Sync + 'static) {
        self.error.as_dyn_error()
    }

    /// Returns a list of iterators over the contexts added to this error.
    pub fn contexts(&self) -> impl Iterator<Item = ContextView<'a>> + 'a {
        self.error
            .inner
            .additional_contexts
            .iter()
            .map(|context| ContextView { context })
    }

    /// An iterator over the immediate children of this error.
    pub fn causes(&self) -> impl Iterator<Item = ErrorView<'a>> + 'a {
        self.error
            .causes()
            .iter()
            .map(|frame| ErrorView { error: frame })
    }

    /// Finds all err-like values in the cause tree of the given type.
    ///
    /// This is most useful for gathering debugging information.
    pub fn find_errors<E2>(&self) -> impl Iterator<Item = TypedErrorView<'a, E2>> + 'a
    where
        E2: Kind,
    {
        self.error.find_errors::<E2>()
    }

    /// Returns an iterator to all the causes of this error, not including the
    /// current error.
    pub fn all_causes(&self) -> impl Iterator<Item = ErrorView<'a>> + 'a {
        self.error.all_causes()
    }
}

impl fmt::Display for ErrorView<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.display_fmt(f)
    }
}

impl fmt::Debug for ErrorView<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.debug_fmt(f)
    }
}
