use std::{
    borrow::Cow,
    error::Error as StdError,
    fmt::{self},
    panic::Location,
};

trait IntoMessage {
    fn into_message(self) -> Cow<'static, str>;
}

impl IntoMessage for String {
    fn into_message(self) -> Cow<'static, str> {
        Cow::Owned(self)
    }
}

impl IntoMessage for &str {
    fn into_message(self) -> Cow<'static, str> {
        Cow::Owned(self.to_string())
    }
}

impl IntoMessage for Cow<'static, str> {
    fn into_message(self) -> Cow<'static, str> {
        self
    }
}

impl IntoMessage for std::fmt::Arguments<'_> {
    fn into_message(self) -> Cow<'static, str> {
        if let Some(static_str) = self.as_str() {
            Cow::Borrowed(static_str)
        } else {
            Cow::Owned(self.to_string())
        }
    }
}

struct Context {
    created_at: &'static Location<'static>,
    message: Cow<'static, str>,
}

struct ErrPayload {
    created_at: &'static Location<'static>,
    error: Box<dyn StdError + Send + Sync>,
    children: Vec<Frame>,
}

struct Frame {
    // We will either have at least one context, or a payload. We may have both.
    /// Contexts on top of the given error. In reverse order of creation.
    contexts: Vec<Context>,

    /// The payload of this frame.
    payload: Option<ErrPayload>,
}

impl Frame {
    #[track_caller]
    fn from_context(context: impl IntoMessage) -> Self {
        Self {
            contexts: vec![Context {
                created_at: Location::caller(),
                message: context.into_message(),
            }],
            payload: None,
        }
    }

    #[track_caller]
    fn from_error<E>(error: E, children: impl IntoIterator<Item = Frame>) -> Self
    where
        E: StdError + Send + Sync + 'static,
    {
        Self {
            contexts: vec![],
            payload: Some(ErrPayload {
                created_at: Location::caller(),
                error: Box::new(error),
                children: children.into_iter().collect(),
            }),
        }
    }

    #[track_caller]
    fn from_root_error<E>(error: E) -> Self
    where
        E: StdError + Send + Sync + 'static,
    {
        Self::from_error(error, std::iter::empty())
    }

    #[track_caller]
    fn add_context(mut self, message: impl IntoMessage) -> Self {
        self.contexts.push(Context {
            created_at: Location::caller(),
            message: message.into_message(),
        });
        self
    }

    fn try_error_ref<E>(&self) -> Option<&E>
    where
        E: StdError + Send + Sync + 'static,
    {
        self.payload.as_ref()?.error.downcast_ref()
    }

    fn try_error_mut<E>(&mut self) -> Option<&mut E>
    where
        E: StdError + Send + Sync + 'static,
    {
        self.payload.as_mut()?.error.downcast_mut()
    }

    fn try_into_error<E>(self) -> Option<E>
    where
        E: StdError + Send + Sync + 'static,
    {
        self.payload
            .and_then(|payload| Some(*payload.error.downcast::<E>().ok()?))
    }

    fn debug_fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let debug: &dyn fmt::Debug = if let Some(context) = self.contexts.last() {
            &context.message
        } else {
            self.payload.as_ref().unwrap().error.as_ref()
        };

        debug.fmt(fmt)
    }

    fn display_fn(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let display: &dyn fmt::Display = if let Some(context) = self.contexts.last() {
            &context.message
        } else {
            self.payload.as_ref().unwrap().error.as_ref()
        };

        display.fmt(fmt)
    }

    fn children(&self) -> &[Frame] {
        if let Some(err_payload) = self.payload.as_ref() {
            err_payload.children.as_slice()
        } else {
            &[]
        }
    }

    fn location(&self) -> &'static Location<'static> {
        if let Some(context) = self.contexts.last() {
            context.created_at
        } else {
            self.payload.as_ref().unwrap().created_at
        }
    }

    fn err_location(&self) -> Option<&'static Location<'static>> {
        self.payload.as_ref().map(|payload| payload.created_at)
    }
}

struct FrameIter<'a> {
    frame_stack: Vec<&'a [Frame]>,
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

        self.frame_stack.push(next_frame.children());

        Some(next_frame)
    }
}

pub(crate) struct ContextView<'a> {
    context: &'a Context,
}

impl ContextView<'_> {
    #[cfg_attr(not(test), expect(dead_code, reason = "experimental"))]
    pub(crate) fn message(&self) -> &str {
        &self.context.message
    }

    #[cfg_attr(not(test), expect(dead_code, reason = "experimental"))]
    pub(crate) fn location(&self) -> &'static Location<'static> {
        self.context.created_at
    }
}

/// A view on an element of an Exn that is known to be of a specific error type.
pub(crate) struct TypedErrorView<'a, E>
where
    E: StdError + Send + Sync + 'static,
{
    // Precondition: error is a frame with a payload of type E.
    error: &'a Frame,
    _phantom: std::marker::PhantomData<E>,
}

impl<E> TypedErrorView<'_, E>
where
    E: StdError + Send + Sync + 'static,
{
    #[cfg_attr(not(test), expect(dead_code, reason = "experimental"))]
    pub(crate) fn error(&self) -> &E {
        self.error.try_error_ref().unwrap()
    }

    #[cfg_attr(not(test), expect(dead_code, reason = "experimental"))]
    pub(crate) fn location(&self) -> &'static Location<'static> {
        self.error.location()
    }

    #[cfg_attr(not(test), expect(dead_code, reason = "experimental"))]
    pub(crate) fn err_location(&self) -> &'static Location<'static> {
        self.error.err_location().expect("Preconditions violated")
    }

    #[cfg_attr(not(test), expect(dead_code, reason = "experimental"))]
    pub(crate) fn contexts(&self) -> impl Iterator<Item = ContextView<'_>> {
        self.error
            .contexts
            .iter()
            .map(|context| ContextView { context })
    }
}

pub(crate) struct ErrorView<'a> {
    error: &'a Frame,
}

impl ErrorView<'_> {
    pub(crate) fn try_as_error<E>(&self) -> Option<&E>
    where
        E: StdError + Send + Sync + 'static,
    {
        self.error.try_error_ref()
    }

    #[cfg_attr(not(test), expect(dead_code, reason = "experimental"))]
    pub(crate) fn has_error_type<E>(&self) -> bool
    where
        E: StdError + Send + Sync + 'static,
    {
        self.try_as_error::<E>().is_some()
    }

    #[cfg_attr(not(test), expect(dead_code, reason = "experimental"))]
    pub(crate) fn location(&self) -> &'static Location<'static> {
        self.error.location()
    }

    #[cfg_attr(not(test), expect(dead_code, reason = "experimental"))]
    pub(crate) fn contexts(&self) -> impl Iterator<Item = ContextView<'_>> {
        self.error
            .contexts
            .iter()
            .map(|context| ContextView { context })
    }
}

impl fmt::Display for ErrorView<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.display_fn(f)
    }
}

impl fmt::Debug for ErrorView<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.debug_fmt(f)
    }
}

pub(crate) struct Exn<E>
where
    E: StdError + Send + Sync + 'static,
{
    root: Box<Frame>,
    _phantom: std::marker::PhantomData<E>,
}

impl<E> Exn<E>
where
    E: StdError + Send + Sync + 'static,
{
    #[cfg_attr(not(test), expect(dead_code, reason = "experimental"))]
    #[track_caller]
    pub(crate) fn new(inner: E) -> Self {
        Self {
            root: Box::new(Frame::from_root_error(inner)),
            _phantom: std::marker::PhantomData,
        }
    }

    #[cfg_attr(not(test), expect(dead_code, reason = "experimental"))]
    #[track_caller]
    pub(crate) fn add_context(self, message: impl Into<String>) -> Self {
        // Does not change the type of the error, as the root error is still in
        // the tree trunk.
        Self {
            root: Box::new(self.root.add_context(message.into())),
            _phantom: self._phantom,
        }
    }

    #[cfg_attr(not(test), expect(dead_code, reason = "experimental"))]
    pub(crate) fn as_error(&self) -> &E {
        self.root
            .try_error_ref()
            .expect("Exn should always contain an error of type E")
    }

    #[cfg_attr(not(test), expect(dead_code, reason = "experimental"))]
    pub(crate) fn as_error_mut(&mut self) -> &mut E {
        self.root
            .try_error_mut()
            .expect("Exn should always contain an error of type E")
    }

    #[cfg_attr(not(test), expect(dead_code, reason = "experimental"))]
    pub(crate) fn into_error(self) -> E {
        self.root
            .try_into_error()
            .expect("Exn should always contain an error of type E")
    }

    fn frames(&self) -> impl Iterator<Item = &'_ Frame> {
        FrameIter {
            frame_stack: vec![std::slice::from_ref(&*self.root)],
        }
    }

    #[cfg_attr(not(test), expect(dead_code, reason = "experimental"))]
    pub(crate) fn find_errors<E2>(&self) -> impl Iterator<Item = TypedErrorView<'_, E2>>
    where
        E2: StdError + Send + Sync + 'static,
    {
        self.frames().filter_map(|frame| {
            if frame.try_error_ref::<E2>().is_some() {
                Some(TypedErrorView {
                    error: frame,
                    _phantom: std::marker::PhantomData,
                })
            } else {
                None
            }
        })
    }

    #[cfg_attr(not(test), expect(dead_code, reason = "experimental"))]
    pub(crate) fn all_children(&self) -> impl Iterator<Item = ErrorView<'_>> {
        self.frames().map(|frame| ErrorView { error: frame })
    }
}

impl<E> fmt::Display for Exn<E>
where
    E: fmt::Display + StdError + Send + Sync + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.root.display_fn(f)
    }
}

impl<E> fmt::Debug for Exn<E>
where
    E: fmt::Debug + StdError + Send + Sync + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.root.debug_fmt(f)
    }
}

pub(crate) struct AnyExn {
    root: Box<Frame>,
}

impl AnyExn {
    #[cfg_attr(not(test), expect(dead_code, reason = "experimental"))]
    pub(crate) fn new(inner: impl fmt::Display) -> Self {
        Self {
            root: Box::new(Frame::from_context(format_args!("{inner}"))),
        }
    }
}

impl fmt::Display for AnyExn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.root.display_fn(f)
    }
}

impl fmt::Debug for AnyExn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.root.debug_fmt(f)
    }
}

impl<E> From<Exn<E>> for AnyExn
where
    E: StdError + Send + Sync + 'static,
{
    fn from(value: Exn<E>) -> Self {
        Self { root: value.root }
    }
}

#[cfg_attr(not(test), expect(unused_macros, reason = "experimental"))]
macro_rules! bail {
    ($fmt:literal $(, $($args:expr),*)? $(,)?) => {
        return Err($crate::helpers::errors::AnyExn::from_arguments(format_args!($fmt $(, $($args),*)?)).into())
    };
    ($err_expr:expr) => {
        return Err($crate::helpers::errors::Exn::new($err_expr).into())
    };
    ($err_expr:expr, $fmt:literal $(, $($args:expr),*)? $(,)?) => {
        return Err($crate::helpers::errors::Exn::new($err_expr).add_context(format_args!($fmt $(, $($args),*)?)).into())
    }
}

#[cfg_attr(not(test), expect(unused_macros, reason = "experimental"))]
macro_rules! ensure {
    ($cond:expr, $($bail_tt:tt)*) => {
        if !$cond {
            $crate::helpers::errors::bail!($($bail_tt)*);
        }
    }
}

#[cfg_attr(not(test), expect(unused_imports, reason = "experimental"))]
pub(crate) use bail;
#[expect(unused_imports, reason = "experimental")]
pub(crate) use ensure;

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, thiserror::Error)]
    #[error("test error")]
    struct TestError;

    mod exn {
        use super::*;

        #[test]
        fn test_new() {
            let err = Exn::new(TestError);
            assert_eq!(format!("{err}"), "test error");
        }

        #[test]
        fn test_as_error() {
            let err = Exn::new(TestError);
            assert!(matches!(err.as_error(), &TestError));
        }

        #[test]
        fn test_as_error_mut() {
            let mut err = Exn::new(TestError);
            assert!(matches!(err.as_error_mut(), &mut TestError));
        }

        #[test]
        fn test_into_error() {
            let err = Exn::new(TestError);
            assert!(matches!(err.into_error(), TestError));
        }

        #[test]
        fn test_add_context() {
            let err = Exn::new(TestError);
            let err = err.add_context("test context");
            assert_eq!(format!("{err}"), "test context");
        }
    }

    mod any_exn {
        use super::*;

        #[test]
        fn test_new() {
            let err = AnyExn::new(TestError);
            assert_eq!(format!("{err}"), "test error");
        }
    }

    mod macros {
        use super::*;

        #[test]
        fn test_bail_for_any_exn() {
            fn err_fn() -> Result<(), AnyExn> {
                bail!(TestError);
            }

            let err = err_fn().unwrap_err();
            assert_eq!(format!("{err}"), "test error");
        }

        #[test]
        fn test_bail_for_exn() {
            fn err_fn() -> Result<(), Exn<TestError>> {
                bail!(TestError);
            }

            let err = err_fn().unwrap_err();
            assert_eq!(format!("{err}"), "test error");
        }

        #[test]
        fn test_ensure_true() {
            fn err_fn() -> Result<(), Exn<TestError>> {
                ensure!(true, TestError);
                Ok(())
            }

            assert!(err_fn().is_ok());
        }

        #[test]
        fn test_ensure_false() {
            fn err_fn() -> Result<(), Exn<TestError>> {
                ensure!(false, TestError);
                Ok(())
            }

            let err = err_fn().unwrap_err();
            assert_eq!(format!("{err}"), "test error");
        }
    }

    mod child_iters {
        use super::*;

        #[derive(Default)]
        struct ErrorBuilder {
            sub_frames: Vec<Frame>,
        }

        impl ErrorBuilder {
            fn new() -> Self {
                Self::default()
            }

            fn add_error(mut self, error: impl Into<AnyExn>) -> Self {
                self.sub_frames.push(*error.into().root);
                self
            }

            fn build<E>(self, err: E) -> Exn<E>
            where
                E: StdError + Send + Sync + 'static,
            {
                let root = Frame::from_error(err, self.sub_frames);
                Exn {
                    root: Box::new(root),
                    _phantom: std::marker::PhantomData,
                }
            }
        }

        macro_rules! build_exn {
            (
                $err_expr:expr
                $(, contexts = [$($context_expr:expr),* $(,)?])?
                $(, [$($sub_expr:expr),* $(,)?])? $(,)?
            ) => {
                ErrorBuilder::new()
                    $($(.add_error($sub_expr))*)?
                    .build($err_expr)
                    $($(.add_context($context_expr))*)?
            }
        }

        #[derive(Debug, thiserror::Error)]
        #[error("needle error")]
        struct NeedleError;

        #[derive(Debug, thiserror::Error)]
        #[error("straw error")]
        struct StrawError;

        #[test]
        fn test_find_errors_with_no_needle() {
            let exn = build_exn!(StrawError);
            let errors: Vec<_> = exn.find_errors::<NeedleError>().collect();
            assert_eq!(errors.len(), 0);
        }

        #[test]
        fn test_find_root_error() {
            let exn = build_exn!(NeedleError);
            let errors: Vec<_> = exn.find_errors::<NeedleError>().collect();
            assert_eq!(errors.len(), 1);
        }

        #[test]
        fn test_find_child_error() {
            let exn = build_exn!(StrawError, [build_exn!(NeedleError)]);
            let errors: Vec<_> = exn.find_errors::<NeedleError>().collect();
            assert_eq!(errors.len(), 1);
        }

        #[test]
        fn test_find_multiple_errors() {
            let exn = build_exn!(
                StrawError,
                [
                    build_exn!(NeedleError),
                    build_exn!(
                        StrawError,
                        contexts = ["straw context"],
                        [build_exn!(NeedleError, contexts = ["needle context"])]
                    )
                ]
            );
            let errors: Vec<_> = exn.find_errors::<NeedleError>().collect();
            assert_eq!(errors.len(), 2);
            assert!(
                errors
                    .iter()
                    .map(TypedErrorView::err_location)
                    .all(|loc| loc.file().ends_with("helpers/errors.rs"))
            );
            assert!(
                errors
                    .iter()
                    .map(TypedErrorView::location)
                    .all(|loc| loc.file().ends_with("helpers/errors.rs"))
            );
            // We will find one context on NeedleError. The other is not found
            // because it is on a different error kind.
            assert!(
                errors
                    .iter()
                    .flat_map(TypedErrorView::contexts)
                    .all(|ctx| ctx.message() == "needle context"
                        && ctx.location().file().ends_with("helpers/errors.rs"))
            );
            assert!(
                errors
                    .iter()
                    .map(TypedErrorView::error)
                    .all(|err| matches!(err, NeedleError))
            );
        }

        #[test]
        fn test_find_all_children() {
            let exn = build_exn!(
                StrawError,
                [
                    build_exn!(NeedleError),
                    build_exn!(
                        StrawError,
                        contexts = ["straw context"],
                        [build_exn!(NeedleError, contexts = ["needle context"])]
                    )
                ]
            );
            let errors: Vec<_> = exn.all_children().collect();
            assert_eq!(errors.len(), 4);
            assert_eq!(
                errors
                    .iter()
                    .filter(|err| err.has_error_type::<NeedleError>())
                    .count(),
                2
            );
            assert_eq!(
                errors
                    .iter()
                    .filter(|err| err.try_as_error::<StrawError>().is_some())
                    .count(),
                2
            );
            assert_eq!(errors.iter().flat_map(ErrorView::contexts).count(), 2);
            assert!(
                errors
                    .iter()
                    .all(|err| err.location().file().ends_with("helpers/errors.rs"))
            );
        }

        #[test]
        fn test_find_all_children_has_error_type() {
            let exn = build_exn!(
                StrawError,
                [
                    build_exn!(NeedleError),
                    build_exn!(StrawError, [build_exn!(NeedleError)])
                ]
            );
            let errors: Vec<_> = exn
                .all_children()
                .filter(ErrorView::has_error_type::<NeedleError>)
                .collect();
            assert_eq!(errors.len(), 2);
        }
    }
}
