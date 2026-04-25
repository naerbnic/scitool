use std::{
    any::Any,
    fmt::{self, Debug, Display},
    marker::PhantomData,
};

use crate::{
    ContextBinder, IntoCause, RaisedMessage, Reportable,
    binders::{Bind, ContextBind, RaiseBinder, ValueBind, ValueContextBind},
    causes::Cause,
    finding::{KindFinding, MessageFinding},
    frame::{ErrorView, Frame},
    locations::SourceLoc,
    out,
    sealed::DiagLikePriv,
};

/// A marker trait for types that are usable as kinds for Diag types.
///
/// This trait has fewer restrictions than [`std::error::Error`], in that these
/// types are not responsible for carrying all of the extra information
/// connected with an error. In particular, Kinds do _not_ need to be
/// [`fmt::Display`]able, as long as any site that raises an error of this
/// kind also provides how the message should appear.
pub trait Kind: Any + fmt::Debug + Send + Sync + 'static {}

// An infallible Kind is a kind that can never be constructed.
impl Kind for std::convert::Infallible {}

#[must_use]
pub struct DiagBuilder<K> {
    frames: Vec<Frame>,
    created_at: SourceLoc,
    _phantom: PhantomData<K>,
}

impl<K> DiagBuilder<K>
where
    K: Kind,
{
    #[track_caller]
    fn new<C>(causes: impl IntoIterator<Item = C>) -> Self
    where
        C: IntoCause,
    {
        let created_at = SourceLoc::current();
        Self {
            frames: causes
                .into_iter()
                .map(|c| c.into_cause(created_at.clone()).into_frame())
                .collect(),
            created_at,
            _phantom: PhantomData,
        }
    }

    fn into_diag(self, fnd: KindFinding<K>) -> Diag<K> {
        Diag::from_finding_and_frames(fnd, self.frames, self.created_at)
    }

    #[must_use]
    pub fn kind(self, kind: K) -> Diag<K>
    where
        K: Reportable,
    {
        self.into_diag(KindFinding::new_kind(kind))
    }

    #[must_use]
    pub fn kind_msg<M>(self, kind: K, msg: M) -> Diag<K>
    where
        M: Reportable,
    {
        self.into_diag(KindFinding::new_kind_msg(kind, msg))
    }

    #[must_use]
    pub fn kind_args(self, kind: K, args: std::fmt::Arguments<'_>) -> Diag<K> {
        self.into_diag(KindFinding::new_kind_args(kind, args))
    }
}

/// An error-like type that provides a typed error value.
///
/// This error type can collect additional context information. It provides
/// error locations based on where the context was added in client code.
///
/// Each `Diag` type is part of a causal tree. At the top is a primary
/// error with the error type `E`, with additional layers of context added
/// on top of it. Below that layer are one or more causes of this error, each
/// with their own context.
pub struct Diag<K>
where
    K: Kind,
{
    root: Frame,
    _phantom: std::marker::PhantomData<K>,
}

impl<K> Diag<K>
where
    K: Kind,
{
    /// Creates a new Diag error based on the given err-like.
    ///
    /// The caller of this function is recorded as the source of the error.
    #[track_caller]
    #[expect(clippy::new_ret_no_self, reason = "DiagBuilder is a builder")]
    pub fn new() -> DiagBuilder<K> {
        DiagBuilder::new(std::iter::empty::<std::convert::Infallible>())
    }

    /// Creates a new Diag error based on the given err-like.
    ///
    /// The caller of this function is recorded as the source of the error.
    #[track_caller]
    pub fn with_causes<C: IntoCause>(causes: impl IntoIterator<Item = C>) -> DiagBuilder<K> {
        DiagBuilder::new(causes)
    }

    pub(crate) fn from_finding_and_causes<C: IntoCause>(
        fnd: KindFinding<K>,
        causes: impl IntoIterator<Item = C>,
        created_at: SourceLoc,
    ) -> Self {
        Self::from_finding_and_frames(
            fnd,
            causes
                .into_iter()
                .map(|c| c.into_cause(created_at.clone()).into_frame())
                .collect(),
            created_at,
        )
    }

    pub(crate) fn from_finding_with_appended_cause<C: IntoCause>(
        fnd: KindFinding<K>,
        cause: C,
        created_at: SourceLoc,
    ) -> Self {
        let cause = cause.into_cause(created_at.clone());
        let weak_cause_msg = cause.msg_clone_weak();
        Self::from_finding_and_frames(
            fnd.append_reportable(weak_cause_msg),
            [cause.into_frame()].into_iter().collect(),
            created_at,
        )
    }

    pub(crate) fn from_finding_and_frames(
        fnd: KindFinding<K>,
        causes: Vec<Frame>,
        created_at: SourceLoc,
    ) -> Self {
        Self {
            root: Frame::new(fnd.into_handle(), created_at, causes),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<K> Diag<K>
where
    K: Kind,
{
    /// Returns the contained err-like type the Diag was created with.
    #[must_use]
    pub fn kind(&self) -> &K {
        self.root
            .try_kind_ref()
            .expect("Diag should always contain an error of type E")
    }

    /// Unwraps this error into the contained err-like.
    ///
    /// This returns the contained error value, as well as an [`AnyDiag`] that
    /// contains the error tree with the root replaced with a placeholder.
    /// This can still be used and thrown, but the object no longer exists
    /// in the tree.
    ///
    /// If you only want the error, you can call `err.into_kind().0`
    #[must_use]
    pub fn into_kind(mut self) -> (K, AnyDiag) {
        let error = self
            .root
            .try_extract_kind()
            .expect("Diag should always contain an error of type E");
        (error, AnyDiag { root: self.root })
    }
}

impl<T> IntoCause for Diag<T>
where
    T: Kind,
{
    fn into_cause(self, _created_at: SourceLoc) -> Cause {
        Cause::from_frame(self.root)
    }
}

impl<E> fmt::Display for Diag<E>
where
    E: Kind,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.root.display_fmt(f)
    }
}

impl<E> fmt::Debug for Diag<E>
where
    E: Kind,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.root.debug_fmt(f)
    }
}

#[must_use]
pub struct AnyDiagBuilder {
    causes: Vec<Frame>,
    created_at: SourceLoc,
}

impl AnyDiagBuilder {
    #[track_caller]
    pub fn new<C>(causes: impl IntoIterator<Item = C>) -> Self
    where
        C: IntoCause,
    {
        let created_at = SourceLoc::current();
        Self {
            causes: causes
                .into_iter()
                .map(|c| c.into_cause(created_at.clone()).into_frame())
                .collect(),
            created_at,
        }
    }

    fn into_msg_finding(self, msg: MessageFinding) -> AnyDiag {
        AnyDiag::from_finding_and_frames(msg, self.causes, self.created_at)
    }

    #[must_use]
    pub fn msg<M>(self, msg: M) -> AnyDiag
    where
        M: Reportable,
    {
        self.into_msg_finding(MessageFinding::new_msg(msg))
    }

    #[must_use]
    pub fn args(self, args: std::fmt::Arguments<'_>) -> AnyDiag {
        self.into_msg_finding(MessageFinding::new_args(args))
    }
}

/// An error like type that provides an untyped error-like value.
///
/// This acts mostly as a type-erased [`Diag`] instance. It should generally
/// be used for cases where the actual type of the error does not matter, but
/// error context should still be preserved.
pub struct AnyDiag {
    root: Frame,
}

impl AnyDiag {
    /// Create a new [`AnyDiag`] from a message-like value.
    #[track_caller]
    #[expect(
        clippy::new_ret_no_self,
        reason = "Using an alternate and short syntax in fluent API"
    )]
    pub fn new() -> AnyDiagBuilder {
        AnyDiagBuilder::new(std::iter::empty::<std::convert::Infallible>())
    }

    #[track_caller]
    pub fn from_std_error<E>(err: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self {
            root: Frame::from_box_std_error(Box::new(err), SourceLoc::current()),
        }
    }

    /// Creates a new Diag error based on the given message-like value.
    ///
    /// The caller of this function is recorded as the source of the error.
    #[track_caller]
    pub fn with_causes<C: IntoCause>(causes: impl IntoIterator<Item = C>) -> AnyDiagBuilder {
        AnyDiagBuilder::new(causes)
    }

    pub(crate) fn from_finding_and_causes<C>(
        fnd: MessageFinding,
        causes: impl IntoIterator<Item = C>,
        created_at: SourceLoc,
    ) -> Self
    where
        C: IntoCause,
    {
        Self::from_finding_and_frames(
            fnd,
            causes
                .into_iter()
                .map(|c| c.into_cause(created_at.clone()).into_frame())
                .collect(),
            created_at,
        )
    }

    pub(crate) fn from_finding_with_appended_cause<C: IntoCause>(
        fnd: MessageFinding,
        cause: C,
        created_at: SourceLoc,
    ) -> Self {
        let cause = cause.into_cause(created_at.clone());
        let weak_cause_msg = cause.msg_clone_weak();
        Self::from_finding_and_frames(
            fnd.append_reportable(weak_cause_msg),
            [cause.into_frame()].into_iter().collect(),
            created_at,
        )
    }

    pub(crate) fn from_finding_and_frames(
        fnd: MessageFinding,
        causes: Vec<Frame>,
        created_at: SourceLoc,
    ) -> Self {
        Self {
            root: Frame::new(fnd.into_err_like(), created_at, causes),
        }
    }

    pub(crate) fn frame(&self) -> &Frame {
        &self.root
    }
}

impl IntoCause for AnyDiag {
    fn into_cause(self, _created_at: SourceLoc) -> Cause {
        Cause::from_frame(self.root)
    }
}

impl fmt::Display for AnyDiag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.root.display_fmt(f)
    }
}

impl fmt::Debug for AnyDiag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.root.debug_fmt(f)
    }
}

impl<K> From<Diag<K>> for AnyDiag
where
    K: Kind,
{
    fn from(value: Diag<K>) -> Self {
        Self { root: value.root }
    }
}

/// An error that is either an actionable error of kind K, or is unactionable.
pub struct MaybeDiag<K>
where
    K: Kind,
{
    root: Frame,
    /// If true, then this error is from an actionable source, and can be
    /// used and extracted.
    ///
    /// Precondition: If this value is true, then the Frame must contain an
    /// error of type E.
    actionable: bool,
    _phantom: PhantomData<K>,
}

impl<K> MaybeDiag<K>
where
    K: Kind,
{
    #[must_use]
    pub fn opt_kind(&self) -> Option<&K> {
        if !self.actionable {
            return None;
        }

        self.root.try_kind_ref::<K>()
    }

    /// Extracts the contained kind if it is actionable, and returns an
    /// [`AnyDiag`] that keeps the rest of the error tree, replacing the
    /// kind with a placeholder.
    #[must_use]
    pub fn into_kind_opt(mut self) -> (Option<K>, AnyDiag) {
        if !self.actionable {
            return (None, AnyDiag { root: self.root });
        }

        let error = self
            .root
            .try_extract_kind()
            .expect("If actionable, frame must contain this type.");

        (Some(error), AnyDiag { root: self.root })
    }
}

impl<K> IntoCause for MaybeDiag<K>
where
    K: Kind,
{
    fn into_cause(self, _created_at: SourceLoc) -> Cause {
        Cause::from_frame(self.root)
    }
}

impl<K> fmt::Display for MaybeDiag<K>
where
    K: Kind,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.root.display_fmt(f)
    }
}

impl<K> fmt::Debug for MaybeDiag<K>
where
    K: Kind,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.root.debug_fmt(f)
    }
}

impl<K> From<Diag<K>> for MaybeDiag<K>
where
    K: Kind,
{
    fn from(value: Diag<K>) -> Self {
        Self {
            root: value.root,
            actionable: true,
            _phantom: PhantomData,
        }
    }
}

impl<K> From<AnyDiag> for MaybeDiag<K>
where
    K: Kind,
{
    fn from(value: AnyDiag) -> Self {
        Self {
            root: value.root,
            actionable: false,
            _phantom: PhantomData,
        }
    }
}

pub trait DiagLike: DiagLikePriv + Display + Debug + Send + Sync + Sized + 'static {
    type Kind: Kind;

    /// Adds a reportable value as a context, return a value of the same
    /// type.
    #[must_use]
    fn add_context(self) -> ContextBinder<impl ContextBind<Out = Self>>;

    /// Raises a new error with type E, with the current error as the previous
    /// link in the causal chain.
    #[must_use]
    fn raise(self) -> RaiseBinder<impl Bind<Out = out::Value>>;

    /// Tries to extract this [`DiagLike`] as an actionable [`Diag<K>`]. If
    /// that fails, it is returned as an unactionable [`AnyDiag`].
    fn extract_actionable(self) -> Result<Diag<Self::Kind>, AnyDiag>;

    /// Returns an equivalent diag as self.
    fn into_any_diag(self) -> AnyDiag {
        match self.extract_actionable() {
            Ok(diag) => diag.into(),
            Err(diag) => diag,
        }
    }

    /// Return an [`crate::ErrorView`] of this [`DiagLike`], which allows for
    /// examination of the contents of the error tree, without the
    /// error-handling semantics of the *Diag types.
    #[must_use]
    fn view(&self) -> ErrorView<'_>;
}

impl<K> DiagLike for Diag<K>
where
    K: Kind,
{
    type Kind = K;

    #[track_caller]
    fn add_context(self) -> ContextBinder<impl ContextBind<Out = Self>> {
        ContextBinder::new(ValueContextBind::new(self))
    }

    #[track_caller]
    fn raise(self) -> RaiseBinder<impl Bind<Out = out::Value>> {
        RaiseBinder::new(ValueBind::new(self))
    }

    fn extract_actionable(self) -> Result<Diag<K>, AnyDiag> {
        Ok(self)
    }

    fn into_any_diag(self) -> AnyDiag {
        AnyDiag { root: self.root }
    }

    fn view(&self) -> ErrorView<'_> {
        self.root.view()
    }
}

impl<K> DiagLikePriv for Diag<K>
where
    K: Kind,
{
    fn add_context_message(&mut self, msg: RaisedMessage) {
        msg.add_to_frame_as_context(&mut self.root);
    }
}

impl DiagLike for AnyDiag {
    type Kind = std::convert::Infallible;

    #[track_caller]
    fn add_context(self) -> ContextBinder<impl ContextBind<Out = Self>> {
        ContextBinder::new(ValueContextBind::new(self))
    }

    #[track_caller]
    fn raise(self) -> RaiseBinder<impl Bind<Out = out::Value>> {
        RaiseBinder::new(ValueBind::new(self))
    }

    fn extract_actionable(self) -> Result<Diag<Self::Kind>, AnyDiag> {
        Err(self)
    }

    fn into_any_diag(self) -> AnyDiag {
        self
    }

    fn view(&self) -> ErrorView<'_> {
        self.root.view()
    }
}

impl DiagLikePriv for AnyDiag {
    fn add_context_message(&mut self, msg: RaisedMessage) {
        msg.add_to_frame_as_context(&mut self.root);
    }
}

impl<K> DiagLike for MaybeDiag<K>
where
    K: Kind,
{
    type Kind = K;

    #[track_caller]
    fn add_context(self) -> ContextBinder<impl ContextBind<Out = Self>> {
        ContextBinder::new(ValueContextBind::new(self))
    }

    #[track_caller]
    fn raise(self) -> RaiseBinder<impl Bind<Out = out::Value>> {
        RaiseBinder::new(ValueBind::new(self))
    }

    fn extract_actionable(self) -> Result<Diag<K>, AnyDiag> {
        if self.actionable {
            Ok(Diag {
                root: self.root,
                _phantom: PhantomData,
            })
        } else {
            Err(AnyDiag { root: self.root })
        }
    }

    fn into_any_diag(self) -> AnyDiag {
        AnyDiag { root: self.root }
    }

    fn view(&self) -> ErrorView<'_> {
        self.root.view()
    }
}

impl<K> DiagLikePriv for MaybeDiag<K>
where
    K: Kind,
{
    fn add_context_message(&mut self, msg: RaisedMessage) {
        msg.add_to_frame_as_context(&mut self.root);
    }
}

impl<K> From<MaybeDiag<K>> for AnyDiag
where
    K: Kind,
{
    fn from(value: MaybeDiag<K>) -> Self {
        value.into_any_diag()
    }
}

/// Creates a [`Diag`] or a [`AnyDiag`] with appropriate context information.
///
/// Calling `diag!` with a format string argument as the first parameter will return an [`AnyDiag`]
/// instance, with the formatted string as the base context.
///
/// ```
/// # use scidev_errors::diag;
/// let name = "Jon Doe";
/// let err = diag!("Hello, {name}!");
/// ```
///
/// Calling `diag!` with a non-literal argument as the first parameter will return a [`Diag`]
/// instance of the same type as the expression. The initial expression can be followed by a format
/// string and arguments, to create a base context for the error.
///
/// As a small sharp edge: In the unlikely event that you provide a literal that is not a string
/// as a first argument, this will cause an error, as we will try to use it as a format string.
/// If you wrap the value in paraenthesis, the macro should work.
#[macro_export]
macro_rules! diag {
    // Function variants that take an error argument, for use with "map_raise()"
    (|$err:ident| message: $msg:expr $(,)?) => {
        |$err, r| r.msg($msg)
    };
    (|$err:ident| $fmt:literal $($arg_tok:tt)*) => {
       |$err, r| r.args(format_args!($fmt $($arg_tok)*))
    };
    (|$err:ident| $err_expr:expr $(,)?) => {
       |$err, r| r.kind($err_expr)
    };
    (|$err:ident| $err_expr:expr, message: $msg:expr $(,)?) => {
       |$err, r| r.kind_msg($err_expr, $msg)
    };
    (|$err:ident| $err_expr:expr, $fmt:literal $($arg_tok:tt)*) => {
       |$err, r| r.kind_args($err_expr, format_args!($fmt $($arg_tok)*))
    };

    // Function variants to immediately yield the appropriate raiser.
    (|| message: $msg:expr $(,)?) => {
        |r| r.msg($msg)
    };
    (|| $fmt:literal $($arg_tok:tt)*) => {
       |r| r.args(format_args!($fmt $($arg_tok)*))
    };
    (|| $err_expr:expr $(,)?) => {
       |r| r.kind($err_expr)
    };
    (|| $err_expr:expr, message: $msg:expr $(,)?) => {
       |r| r.kind_msg($err_expr, $msg)
    };
    (|| $err_expr:expr, $fmt:literal $($arg_tok:tt)*) => {
       |r| r.kind_args($err_expr, format_args!($fmt $($arg_tok)*))
    };

    // Value variants. These evaluate to the explicit diag function.
    (message: $msg:expr $(,)?) => {
        $crate::AnyDiag::new().msg($msg)
    };
    ($fmt:literal $($arg_tok:tt)*) => {
        $crate::AnyDiag::new().args(format_args!($fmt $($arg_tok)*))
    };
    ($err_expr:expr $(,)?) => {
        $crate::Diag::new().kind($err_expr)
    };
    ($err_expr:expr, message: $msg:expr $(,)?) => {
        $crate::Diag::new().kind_msg($err_expr, $msg)
    };
    ($err_expr:expr, $fmt:literal $($arg_tok:tt)*) => {
        $crate::Diag::new().kind_args($err_expr, format_args!($fmt $($arg_tok)*))
    }
}

/// A macro to automatically throw a [`Diag`] or a [`AnyDiag`] with appropriate context information.
///
/// The arguments to [`crate::bail!`] are identical to the [`crate::diag!`] macro. See it for more details.
#[macro_export]
macro_rules! bail {
     ($($diag_tt:tt)*) => {
         return Err($crate::diag!($($diag_tt)*).into())
     };
 }

/// A macro to automatically throw a [`Diag`] or an [`AnyDiag`] with appropriate context
/// information.
///
/// The first argument to [`crate::ensure!`] is a simple boolean expression that will always be evaluated
/// The remaining arguments, are identical to the [`crate::diag!`] macro. See it for more details.
#[macro_export]
macro_rules! ensure {
     ($cond:expr, $($bail_tt:tt)*) => {
         if !$cond {
             $crate::bail!($($bail_tt)*);
         }
     }
 }

#[cfg(test)]
mod tests {
    use crate::TypedErrorView;

    use super::*;

    #[derive(Debug, thiserror::Error)]
    #[error("test error")]
    struct TestError;

    impl Kind for TestError {}

    mod diag {
        use super::*;

        #[test]
        fn test_new() {
            let err = Diag::new().kind(TestError);
            assert_eq!(format!("{err}"), "test error");
        }

        #[test]
        fn test_as_error() {
            let err = Diag::new().kind(TestError);
            assert!(matches!(err.kind(), &TestError));
        }

        #[test]
        fn test_into_error() {
            let err = Diag::new().kind(TestError);
            let (err_val, _) = err.into_kind();
            assert!(matches!(err_val, TestError));
        }

        #[test]
        fn test_add_context() {
            let err = Diag::new().kind(TestError);
            let err = err.add_context().msg("test context");

            assert_eq!(format!("{err:#}"), "test error (test context)");
        }
    }

    mod any_diag {
        use super::*;

        #[test]
        fn test_new() {
            let err = AnyDiag::new().msg(TestError);
            assert_eq!(format!("{err}"), "test error");
        }
    }

    mod macros {
        use super::*;

        #[test]
        fn test_bail_for_any_diag() {
            fn err_fn() -> Result<(), AnyDiag> {
                bail!(TestError);
            }

            let err = err_fn().unwrap_err();
            assert_eq!(format!("{err}"), "test error");
        }

        #[test]
        fn test_bail_for_diag() {
            fn err_fn() -> Result<(), Diag<TestError>> {
                bail!(TestError);
            }

            let err = err_fn().unwrap_err();
            assert_eq!(format!("{err}"), "test error");
        }

        #[test]
        fn test_ensure_true() {
            fn err_fn() -> Result<(), Diag<TestError>> {
                ensure!(true, TestError);
                Ok(())
            }

            assert!(err_fn().is_ok());
        }

        #[test]
        fn test_ensure_false() {
            fn err_fn() -> Result<(), Diag<TestError>> {
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
            sub_frames: Vec<AnyDiag>,
        }

        impl ErrorBuilder {
            fn new() -> Self {
                Self::default()
            }

            fn add_error(mut self, error: impl Into<AnyDiag>) -> Self {
                self.sub_frames.push(error.into());
                self
            }

            #[track_caller]
            fn build<E>(self, err: E) -> Diag<E>
            where
                E: Kind + Reportable,
            {
                Diag::with_causes(self.sub_frames).kind(err)
            }
        }

        macro_rules! build_diag {
             (
                 $err_expr:expr
                 $(, contexts = [$($context_expr:expr),* $(,)?])?
                 $(, [$($sub_expr:expr),* $(,)?])? $(,)?
             ) => {
                 ErrorBuilder::new()
                     $($(.add_error($sub_expr))*)?
                     .build($err_expr)
                     $($(.add_context().msg($context_expr))*)?
             }
         }

        #[derive(Debug, thiserror::Error)]
        #[error("needle error")]
        struct NeedleError;

        impl Kind for NeedleError {}

        #[derive(Debug, thiserror::Error)]
        #[error("straw error")]
        struct StrawError;

        impl Kind for StrawError {}

        #[test]
        fn test_find_errors_with_no_needle() {
            let diag = build_diag!(StrawError);
            let errors: Vec<_> = diag.view().find_kinds::<NeedleError>().collect();
            assert_eq!(errors.len(), 0);
        }

        #[test]
        fn test_find_root_error() {
            let diag = build_diag!(NeedleError);
            let errors: Vec<_> = diag.view().find_kinds::<NeedleError>().collect();
            assert_eq!(errors.len(), 1);
        }

        #[test]
        fn test_find_child_error() {
            let diag = build_diag!(StrawError, [build_diag!(NeedleError)]);
            let errors: Vec<_> = diag.view().find_kinds::<NeedleError>().collect();
            assert_eq!(errors.len(), 1);
        }

        #[test]
        fn test_find_multiple_errors() {
            let diag = build_diag!(
                StrawError,
                [
                    build_diag!(NeedleError),
                    build_diag!(
                        StrawError,
                        contexts = ["straw context"],
                        [build_diag!(NeedleError, contexts = ["needle context"])]
                    )
                ]
            );
            let errors: Vec<_> = diag.view().find_kinds::<NeedleError>().collect();
            assert_eq!(errors.len(), 2);
            assert!(
                errors
                    .iter()
                    .map(TypedErrorView::err_location)
                    .all(|loc| loc.file().ends_with("diag.rs"))
            );
            assert!(
                errors
                    .iter()
                    .map(TypedErrorView::location)
                    .all(|loc| loc.file().ends_with("diag.rs"))
            );
            // We will find one context on NeedleError. The other is not found
            // because it is on a different error kind.
            assert!(
                errors
                    .iter()
                    .flat_map(TypedErrorView::contexts)
                    .all(|ctx| format!("{}", ctx.message()) == "needle context"
                        && ctx.location().file().ends_with("diag.rs"))
            );
            assert!(
                errors
                    .iter()
                    .map(TypedErrorView::kind)
                    .all(|err| matches!(err, NeedleError))
            );
        }

        #[test]
        fn test_find_all_causes() {
            let diag = build_diag!(
                StrawError,
                [
                    build_diag!(NeedleError),
                    build_diag!(
                        StrawError,
                        contexts = ["straw context"],
                        [build_diag!(NeedleError, contexts = ["needle context"])]
                    )
                ]
            );
            // Note that since all_causes only looks at the causes of the current diag, we will
            // miss the first "StrawError".
            let errors: Vec<_> = diag.view().all_causes().collect();
            assert_eq!(errors.len(), 3);
            assert_eq!(
                errors
                    .iter()
                    .filter(|err| err.has_kind_type::<NeedleError>())
                    .count(),
                2
            );
            assert_eq!(
                errors
                    .iter()
                    .filter(|err| err.has_kind_type::<StrawError>())
                    .count(),
                1
            );
            assert_eq!(errors.iter().flat_map(ErrorView::contexts).count(), 2);
            assert!(
                errors
                    .iter()
                    .all(|err| err.location().file().ends_with("diag.rs"))
            );
        }

        #[test]
        fn test_find_all_children_has_error_type() {
            let diag = build_diag!(
                StrawError,
                [
                    build_diag!(NeedleError),
                    build_diag!(StrawError, [build_diag!(NeedleError)])
                ]
            );
            let errors: Vec<_> = diag
                .view()
                .all_causes()
                .filter(ErrorView::has_kind_type::<NeedleError>)
                .collect();
            assert_eq!(errors.len(), 2);
        }
    }

    mod extra_tests {
        use crate::{OptionExt as _, ResultExt as _};

        use super::*;

        #[derive(Debug, PartialEq)]
        struct ErrorA;

        impl std::fmt::Display for ErrorA {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("error A")
            }
        }

        impl Kind for ErrorA {}

        #[derive(Debug, thiserror::Error, PartialEq)]
        #[error("error B")]
        struct ErrorB;

        impl Kind for ErrorB {}

        #[test]
        fn test_diag_raise_error() {
            let diag_a = Diag::new().kind(ErrorA);
            let diag_b = diag_a.raise().kind(ErrorB);

            assert_eq!(format!("{diag_b}"), "error B");
            // Check that ErrorA is a child of ErrorB
            let children: Vec<_> = diag_b.view().all_causes().collect();
            assert!(children.iter().any(ErrorView::has_kind_type::<ErrorA>));
        }

        #[test]
        fn test_any_diag_raise_error() {
            let any_diag = AnyDiag::from(Diag::new().kind(ErrorA));
            let diag_b = any_diag.raise().kind(ErrorB);

            assert_eq!(format!("{diag_b}"), "error B");
            let children: Vec<_> = diag_b.view().all_causes().collect();
            assert!(children.iter().any(ErrorView::has_kind_type::<ErrorA>));
        }

        #[test]
        fn test_into_diag_like() {
            // T -> Diag<T>
            let diag_a = Diag::new().kind(ErrorA);
            assert_eq!(format!("{diag_a}"), "error A");

            // Diag<T> -> Diag<T>
            let diag_a_2 = diag_a.into_any_diag();
            assert_eq!(format!("{diag_a_2}"), "error A");

            // AnyDiag -> AnyDiag
            let any_diag = AnyDiag::new().msg(ErrorA);
            let any_diag_2 = any_diag.into_any_diag();
            assert_eq!(format!("{any_diag_2}"), "error A");
        }

        #[test]
        fn test_result_ext_with_context() {
            // Ok case
            let res: Result<(), Diag<ErrorA>> = Ok(());
            let res = res.with_context().msg("context");
            assert!(res.is_ok());

            // Err case
            let res: Result<(), Diag<ErrorA>> = Err(Diag::new().kind(ErrorA));
            let res = res.with_context().msg("context");
            assert!(res.is_err());
            let diag = res.unwrap_err();

            assert_eq!(format!("{diag:#}"), "error A (context)");
        }

        #[test]
        fn test_result_ext_raise() {
            // Ok case
            let res: Result<(), Diag<ErrorA>> = Ok(());
            let res = res.raise().kind(ErrorB);
            assert!(res.is_ok());

            // Err case
            let res: Result<(), Diag<ErrorA>> = Err(Diag::new().kind(ErrorA));
            let res = res.raise().kind(ErrorB);
            assert!(res.is_err());
            let diag = res.unwrap_err();
            assert_eq!(format!("{diag}"), "error B");

            let children: Vec<_> = diag.view().all_causes().collect();
            assert!(children.iter().any(ErrorView::has_kind_type::<ErrorA>));
        }

        #[test]
        fn test_option_ext_with_raise() {
            // Some case
            let opt: Option<()> = Some(());
            let res = opt.raise().msg("context");
            assert!(res.is_ok());

            // None case
            let opt: Option<()> = None;
            let res = opt.raise().msg("context");
            assert!(res.is_err());
            let any_diag = res.unwrap_err();
            assert_eq!(format!("{any_diag}"), "context");
        }

        #[test]
        fn test_option_ext_raise() {
            // Some case
            let opt: Option<()> = Some(());
            let res = opt.raise().kind(ErrorA);
            assert!(res.is_ok());

            // None case
            let opt: Option<()> = None;
            let res = opt.raise().kind(ErrorA);
            assert!(res.is_err());
            let diag = res.unwrap_err();
            assert_eq!(format!("{diag}"), "error A");
        }
    }
}
