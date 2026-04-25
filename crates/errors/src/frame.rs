mod cursor;

use std::{
    error::Error as StdError,
    fmt::{self, Debug, Display},
    marker::PhantomData,
};

use crate::{
    Kind,
    finding::MessageFinding,
    fmt_helpers::{Indent, indent_fmt},
    locations::SourceLoc,
    reportable::{Reportable, ReportableHandle, WeakReportableHandle},
};

use cursor::FrameCursor;

#[derive(Debug)]
struct Context {
    created_at: SourceLoc,
    message: ReportableHandle,
}

fn format_frame_report(
    fmt_ctxt: &FrameCursor,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    fmt_ctxt.write_message_to(f)?;
    if let Some(created_at) = fmt_ctxt.created_at() {
        write!(f, "\n  at {created_at}")?;
    }
    let additional_contexts = fmt_ctxt.additional_contexts();
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

    let mut causes = fmt_ctxt.causes();
    if causes.len() == 1 {
        let cause = causes.next().unwrap();
        write!(f, "\n  Caused by:\n    ",)?;
        indent_fmt(f, 4, |f| format_frame_report(&cause, f))?;
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
            indent_fmt(f, 6 + cause_char_len, |f| format_frame_report(&cause, f))?;
        }
    }
    Ok(())
}

/// The root error used for the Frame.
#[derive(Debug)]
enum FrameRoot {
    DiagRoot {
        base_context: Context,
        causes: Vec<Frame>,
    },
    StdErrorRoot {
        source_loc: SourceLoc,
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
}

impl FrameRoot {
    fn from_diag(
        root_reportable: ReportableHandle,
        created_at: SourceLoc,
        causes: Vec<Frame>,
    ) -> Self {
        Self::DiagRoot {
            base_context: Context {
                created_at,
                message: root_reportable,
            },
            causes,
        }
    }

    fn from_box_std_error(
        err: Box<dyn std::error::Error + Send + Sync + 'static>,
        created_at: SourceLoc,
    ) -> Self {
        Self::StdErrorRoot {
            source_loc: created_at,
            source: err,
        }
    }

    fn message_clone_weak(&self) -> WeakReportableHandle {
        match self {
            FrameRoot::DiagRoot { base_context, .. } => base_context.message.clone_weak(),
            FrameRoot::StdErrorRoot { .. } => WeakReportableHandle::new_dangling(),
        }
    }

    fn downcast_ref<K>(&self) -> Option<&K>
    where
        K: Kind,
    {
        match self {
            FrameRoot::DiagRoot { base_context, .. } => base_context.message.downcast_ref(),
            FrameRoot::StdErrorRoot { .. } => None,
        }
    }

    fn try_extract_kind<K>(&mut self) -> Option<K>
    where
        K: Kind,
    {
        match self {
            FrameRoot::DiagRoot { base_context, .. } => {
                let placeholder = ReportableHandle::from_report_only("<extracted>");
                let error =
                    std::mem::replace(&mut base_context.message, placeholder).downcast::<K>()?;
                Some(error)
            }
            FrameRoot::StdErrorRoot { .. } => None,
        }
    }

    fn created_at(&self) -> &SourceLoc {
        match self {
            FrameRoot::DiagRoot { base_context, .. } => &base_context.created_at,
            FrameRoot::StdErrorRoot { source_loc, .. } => source_loc,
        }
    }

    fn message(&self) -> &dyn fmt::Display {
        match self {
            FrameRoot::DiagRoot { base_context, .. } => &base_context.message,
            FrameRoot::StdErrorRoot { source, .. } => &**source,
        }
    }

    fn causes(&self) -> &[Frame] {
        match self {
            FrameRoot::DiagRoot { causes, .. } => causes,
            FrameRoot::StdErrorRoot { .. } => &[],
        }
    }
}

#[derive(Debug)]
struct Inner {
    /// Contexts on top of the given error. In reverse order of creation.
    additional_contexts: Vec<Context>,

    /// The root error of this frame.
    frame_root: FrameRoot,
}

impl Inner {
    fn get_cursor(&self) -> FrameCursor<'_> {
        FrameCursor::from_frame(self)
    }
}

#[derive(Debug)]
pub(crate) struct Frame {
    inner: Box<Inner>,
}

impl Frame {
    pub(crate) fn new(
        root_reportable: ReportableHandle,
        created_at: SourceLoc,
        causes: Vec<Frame>,
    ) -> Self {
        Self {
            inner: Box::new(Inner {
                additional_contexts: Vec::new(),
                frame_root: FrameRoot::from_diag(root_reportable, created_at, causes),
            }),
        }
    }

    pub(crate) fn from_box_std_error(
        err: Box<dyn std::error::Error + Send + Sync + 'static>,
        created_at: SourceLoc,
    ) -> Self {
        Self {
            inner: Box::new(Inner {
                additional_contexts: Vec::new(),
                frame_root: FrameRoot::from_box_std_error(err, created_at),
            }),
        }
    }

    pub(crate) fn clone_msg_weak(&self) -> WeakReportableHandle {
        self.inner.frame_root.message_clone_weak()
    }

    pub(crate) fn add_context(&mut self, msg: MessageFinding, created_at: SourceLoc) {
        self.inner.additional_contexts.push(Context {
            created_at,
            message: msg.into_err_like(),
        });
    }

    pub(crate) fn has_kind<E>(&self) -> bool
    where
        E: Kind,
    {
        self.inner.frame_root.downcast_ref::<E>().is_some()
    }

    pub(crate) fn try_kind_ref<E>(&self) -> Option<&E>
    where
        E: Kind,
    {
        self.inner.frame_root.downcast_ref()
    }

    pub(crate) fn try_extract_kind<E>(&mut self) -> Option<E>
    where
        E: Kind,
    {
        self.inner.frame_root.try_extract_kind()
    }

    fn get_cursor(&self) -> FrameCursor<'_> {
        self.inner.get_cursor()
    }

    pub(crate) fn report_fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        format_frame_report(&self.get_cursor(), fmt)
    }

    pub(crate) fn debug_fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        if fmt.alternate() {
            return self.report_fmt(fmt);
        }

        Debug::fmt(self, fmt)
    }

    pub(crate) fn display_fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let fmt_ctxt = self.get_cursor();
        fmt_ctxt.write_message_to(fmt)?; // TODO: Check if we need to differentiate between display and debug

        if fmt.alternate() {
            // Print as "<primary message> (<context #1>, <context #2>, ...)"

            let additional_contexts = fmt_ctxt.additional_contexts();

            if !additional_contexts.is_empty() {
                fmt.write_str(" (")?;
                let mut first = true;
                for ctxt in additional_contexts {
                    if first {
                        first = false;
                    } else {
                        fmt.write_str(", ")?;
                    }
                    write!(fmt, "{}", ctxt.message)?;
                }
                fmt.write_str(")")?;
            }
        }

        Ok(())
    }

    pub(crate) fn causes(&self) -> &[Frame] {
        self.inner.frame_root.causes()
    }

    pub(crate) fn location(&self) -> &SourceLoc {
        if let Some(context) = self.inner.additional_contexts.last() {
            &context.created_at
        } else {
            self.inner.frame_root.created_at()
        }
    }

    /// Returns the root source of this frame's error. If this is a
    /// context-based frame, then this is the location of the originating
    /// context.
    pub(crate) fn kind_location(&self) -> &SourceLoc {
        self.inner.frame_root.created_at()
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

    pub(crate) fn find_kinds<K>(&self) -> impl Iterator<Item = TypedErrorView<'_, K>>
    where
        K: Kind,
    {
        self.all_frames().filter_map(|view| view.as_typed::<K>())
    }

    /// Returns an iterator over all causes of this frame, including the cause
    /// represented by the current [`Frame`]
    pub(crate) fn all_causes(&self) -> impl Iterator<Item = ErrorView<'_>> {
        FrameIter::from_frame_slice(self.inner.frame_root.causes()).map(Frame::view)
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
    pub fn location(&self) -> &SourceLoc {
        &self.context.created_at
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

impl<'a, K> TypedErrorView<'a, K>
where
    K: Kind,
{
    /// Returns the contained error-like value.
    #[must_use]
    pub fn kind(&self) -> &K {
        self.error.try_kind_ref().unwrap()
    }

    /// Returns the last location that context was added.
    #[must_use]
    pub fn location(&self) -> &SourceLoc {
        self.error.location()
    }

    /// Returns the initial location that this error was raised at.
    #[must_use]
    pub fn err_location(&self) -> &SourceLoc {
        self.error.kind_location()
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
    pub fn find_errors<K2>(&self) -> impl Iterator<Item = TypedErrorView<'a, K2>> + 'a
    where
        K2: Kind,
    {
        self.error.find_kinds::<K2>()
    }

    /// Returns an iterator to all the causes of this error, not including the
    /// current error.
    pub fn all_causes(&self) -> impl Iterator<Item = ErrorView<'a>> + 'a {
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
    pub fn as_typed<K>(&self) -> Option<TypedErrorView<'a, K>>
    where
        K: Kind,
    {
        if self.error.has_kind::<K>() {
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
    pub fn has_kind_type<K>(&self) -> bool
    where
        K: Kind,
    {
        self.error.has_kind::<K>()
    }

    /// Returns the last code location that the error had context added to.
    #[must_use]
    pub fn location(&self) -> &SourceLoc {
        self.error.location()
    }

    /// Returns the initial location that this error was raised at.
    #[must_use]
    pub fn kind_location(&self) -> &SourceLoc {
        self.error.kind_location()
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
    pub fn find_kinds<K>(&self) -> impl Iterator<Item = TypedErrorView<'a, K>> + 'a
    where
        K: Kind,
    {
        self.error.find_kinds::<K>()
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
