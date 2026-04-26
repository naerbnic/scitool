mod cursor;

use std::{
    fmt::{self, Debug},
    marker::PhantomData,
};

use crate::{
    Kind,
    dyn_err_conversion::try_convert_box_to_any_diag,
    finding::MessageFinding,
    locations::SourceLoc,
    reportable::{Reportable, ReportableHandle, WeakReportableHandle},
};

use cursor::FrameCursor;

#[derive(Debug)]
struct Context {
    created_at: SourceLoc,
    message: ReportableHandle,
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
}

#[derive(Debug)]
struct Inner {
    /// Contexts on top of the given error. In reverse order of creation.
    additional_contexts: Vec<Context>,

    /// The root error of this frame.
    frame_root: FrameRoot,
}

impl Inner {
    fn location(&self) -> &SourceLoc {
        if let Some(context) = self.additional_contexts.last() {
            &context.created_at
        } else {
            self.frame_root.created_at()
        }
    }

    /// Returns the root source of this frame's error. If this is a
    /// context-based frame, then this is the location of the originating
    /// context.
    fn kind_location(&self) -> &SourceLoc {
        self.frame_root.created_at()
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
        match try_convert_box_to_any_diag(err) {
            Ok(any_diag) => any_diag.into_frame(),
            Err(err) => Self {
                inner: Box::new(Inner {
                    additional_contexts: Vec::new(),
                    frame_root: FrameRoot::from_box_std_error(err, created_at),
                }),
            },
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
        FrameCursor::from_frame(self)
    }

    pub(crate) fn report_fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        self.get_cursor().report_fmt(fmt)
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

    pub(crate) fn location(&self) -> &SourceLoc {
        self.inner.location()
    }

    /// Returns the root source of this frame's error. If this is a
    /// context-based frame, then this is the location of the originating
    /// context.
    pub(crate) fn kind_location(&self) -> &SourceLoc {
        self.inner.kind_location()
    }

    pub(crate) fn view(&self) -> ErrorView<'_> {
        ErrorView {
            cursor: self.get_cursor(),
        }
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
    view: ErrorView<'a>,
    _phantom: std::marker::PhantomData<E>,
}

impl<'a, K> TypedErrorView<'a, K>
where
    K: Kind,
{
    /// Returns the contained error-like value.
    #[must_use]
    pub fn kind(&self) -> &K {
        self.view.cursor.try_kind_ref().unwrap()
    }

    /// Returns the last location that context was added.
    #[must_use]
    pub fn location(&self) -> Option<&SourceLoc> {
        self.view.location()
    }

    /// Returns the initial location that this error was raised at.
    #[must_use]
    pub fn err_location(&self) -> Option<&SourceLoc> {
        self.view.kind_location()
    }

    /// Returns an iterator over the contexts that were added to this error.
    pub fn contexts(&self) -> impl Iterator<Item = ContextView<'a>> + 'a {
        self.view.contexts()
    }

    /// An iterator over the immediate children of this error.
    pub fn causes(&self) -> impl Iterator<Item = ErrorView<'a>> + 'a {
        self.view.causes()
    }

    /// Finds all err-like values in the cause tree of the given type.
    ///
    /// This is most useful for gathering debugging information.
    pub fn find_errors<K2>(&self) -> impl Iterator<Item = TypedErrorView<'a, K2>> + 'a
    where
        K2: Kind,
    {
        self.view.find_kinds::<K2>()
    }

    /// Returns an iterator to all the causes of this error, not including the
    /// current error.
    pub fn all_causes(&self) -> impl Iterator<Item = ErrorView<'a>> + 'a {
        self.view.all_causes()
    }
}

impl<E> fmt::Display for TypedErrorView<'_, E>
where
    E: Kind,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.view, f)
    }
}

impl<E> fmt::Debug for TypedErrorView<'_, E>
where
    E: Kind,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.view, f)
    }
}

/// Returns a view on a raised error whose type is not known.
pub struct ErrorView<'a> {
    cursor: FrameCursor<'a>,
}

impl<'a> ErrorView<'a> {
    fn from_cursor(cursor: FrameCursor<'a>) -> Self {
        Self { cursor }
    }

    /// Returns `Some(err)` for the contained err-like, if the err-like is of
    /// type `E`.
    #[must_use]
    pub fn as_typed<K>(&self) -> Option<TypedErrorView<'a, K>>
    where
        K: Kind,
    {
        if self.has_kind_type::<K>() {
            Some(TypedErrorView {
                view: Self {
                    cursor: self.cursor.clone(),
                },
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
        self.cursor.try_kind_ref::<K>().is_some()
    }

    /// Returns the last code location that the error had context added to.
    #[must_use]
    pub fn location(&self) -> Option<&SourceLoc> {
        self.cursor.location()
    }

    /// Returns the initial location that this error was raised at.
    #[must_use]
    pub fn kind_location(&self) -> Option<&SourceLoc> {
        self.cursor.kind_location()
    }

    /// Returns a list of iterators over the contexts added to this error.
    pub fn contexts(&self) -> impl Iterator<Item = ContextView<'a>> + 'a {
        self.cursor
            .additional_contexts()
            .iter()
            .map(|context| ContextView { context })
    }

    /// An iterator over the immediate children of this error.
    pub fn causes(&self) -> impl Iterator<Item = ErrorView<'a>> + 'a {
        self.cursor
            .causes()
            .map(|frame| ErrorView { cursor: frame })
    }

    /// Finds all err-like values in the cause tree of the given type.
    ///
    /// This is most useful for gathering debugging information.
    pub fn find_kinds<K>(&self) -> impl Iterator<Item = TypedErrorView<'a, K>> + 'a
    where
        K: Kind,
    {
        self.cursor
            .clone()
            .into_all_frames_iter()
            .map(ErrorView::from_cursor)
            .filter_map(|view| view.as_typed::<K>())
    }

    /// Returns an iterator to all the causes of this error, not including the
    /// current error.
    pub fn all_causes(&self) -> impl Iterator<Item = ErrorView<'a>> + 'a {
        let mut iter = self.cursor.clone().into_all_frames_iter();

        // Remove the first item, which should be the root cursor.
        iter.next();

        iter.map(ErrorView::from_cursor)
    }
}

impl fmt::Display for ErrorView<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.cursor.display_fmt(f)
    }
}

impl fmt::Debug for ErrorView<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.cursor.debug_fmt(f)
    }
}
