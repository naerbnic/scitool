/// Defines a custom error struct that wraps a diagnostic.
///
/// This macro generates a new error type with the specified visibility and name.
/// The resulting struct operates as a standard [`std::error::Error`] type.
///
/// There are three options for error types:
///
/// - `define_error! { struct MyErr; }` will define an error type that represents an opaque
///   error. Any [`crate::DiagLike`] type can be converted to it.
/// - `define_error! { struct MyErr { type Kind = MyErrKind; } }` will define an error type that is
///   always actionable. It will have a `kind()` method that returns a
///   reference to `MyErrKind`. Values of `MyErrKind` and
///   `crate::Diag<MyErrKind>` are convertible to it. Note
///   that all error values _must_ have an instance of `MyErrKind`.
/// - `define_error! { struct MyErr { type OptKind = MyErrKind; } }` will define an error type that
///   is _sometimes_ actionable. It has a `kind()` method that returns an
///   optional reference to a `MyErrKind`, intending to differentiate between an
///   actionable and non-actionable error. Can be converted from a `MyErrKind`,
///   a `crate::Diag<MyErrKind>`, or a `crate::AnyDiag`. Note that
///   when converted from an `AnyDiag`, it will _always_ return None from
///   `kind()` even if the underlying type of the `AnyDiag` is an instance of
///   `MyErrKind`.
///
/// The structs defined within a [`crate::define_error`] can have attributes added to
/// it, along with documentation comments as desired.
#[macro_export]
macro_rules! define_error {
    {
        $(#[$meta:meta])* $v:vis struct $name:ident;
    } => {
        $(#[$meta])*
        $v struct $name {
            diag: $crate::AnyDiag,
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Display::fmt(&self.diag, f)
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Debug::fmt(&self.diag, f)
            }
        }

        impl std::error::Error for $name {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                $crate::DiagLike::view(&self.diag).as_dyn_error().source()
            }
        }

        impl std::convert::From<$crate::AnyDiag> for $name {
            fn from(value: $crate::AnyDiag) -> Self {
                Self {
                    diag: value
                }
            }
        }

        // Also converts from all Diag types, with its contents becoming
        // non-actionable.
        impl<E> std::convert::From<$crate::Diag<E>> for $name where E: $crate::Kind {
            fn from(value: $crate::Diag<E>) -> Self {
                Self {
                    diag: value.into()
                }
            }
        }

        impl<E> std::convert::From<$crate::MaybeDiag<E>> for $name where E: $crate::Kind {
            fn from(value: $crate::MaybeDiag<E>) -> Self {
                Self {
                    diag: value.into()
                }
            }
        }

        impl $crate::DiagStdError<$crate::AnyDiag> for $name {
            fn into_diag(self) -> $crate::AnyDiag {
                self.diag
            }
        }

        impl $crate::AnyDiagStdError for $name {
            fn into_any_diag(self) -> $crate::AnyDiag {
                self.diag.into()
            }
        }
    };
    {
        $(#[$meta:meta])* $v:vis struct $name:ident {
            type Kind = $kind:ty;
        }
    } => {
        $(#[$meta])*
        $v struct $name {
            diag: $crate::Diag<$kind>,
        }

        impl $name {
            $v fn kind(&self) -> &$kind {
                self.diag.kind()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Display::fmt(&self.diag, f)
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Debug::fmt(&self.diag, f)
            }
        }

        impl std::error::Error for $name {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                $crate::DiagLike::view(&self.diag).as_dyn_error().source()
            }
        }

        impl std::convert::From<$crate::Diag<$kind>> for $name {
            fn from(value: $crate::Diag<$kind>) -> Self {
                Self {
                    diag: value
                }
            }
        }

        impl std::convert::From<$kind> for $name {
            // Track caller to track where this was first created.
            #[track_caller]
            fn from(value: $kind) -> Self {
                Self {
                    diag: $crate::Diag::new().kind(value)
                }
            }
        }

        impl $crate::DiagStdError<$crate::Diag<$kind>> for $name {
            fn into_diag(self) -> $crate::Diag<$kind> {
                self.diag
            }
        }

        impl $crate::AnyDiagStdError for $name {
            fn into_any_diag(self) -> $crate::AnyDiag {
                self.diag.into()
            }
        }
    };
    {
        $(#[$meta:meta])* $v:vis struct $name:ident {
            type OptKind = $kind:ty;
        }
    } => {
        $(#[$meta])*
        $v struct $name {
            diag: $crate::MaybeDiag<$kind>
        }

        impl $name {
            $v fn opt_kind(&self) -> Option<&$kind> {
                self.diag.opt_kind()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Display::fmt(&self.diag, f)
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Debug::fmt(&self.diag, f)
            }
        }

        impl std::error::Error for $name {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                $crate::DiagLike::view(&self.diag).as_dyn_error().source()
            }
        }

        impl std::convert::From<$crate::AnyDiag> for $name {
            fn from(value: $crate::AnyDiag) -> Self {
                Self {
                    diag: value.into()
                }
            }
        }

        impl std::convert::From<$crate::Diag<$kind>> for $name {
            fn from(value: $crate::Diag<$kind>) -> Self {
                Self {
                    diag: value.into()
                }
            }
        }

        impl std::convert::From<$crate::MaybeDiag<$kind>> for $name {
            fn from(value: $crate::MaybeDiag<$kind>) -> Self {
                Self {
                    diag: value
                }
            }
        }

        impl $crate::DiagStdError<$crate::MaybeDiag<$kind>> for $name {
            fn into_diag(self) -> $crate::MaybeDiag<$kind> {
                self.diag
            }
        }

        impl $crate::AnyDiagStdError for $name {
            fn into_any_diag(self) -> $crate::AnyDiag {
                self.diag.into()
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use crate::{
        Kind,
        diag::{AnyDiag, Diag},
    };

    #[derive(Debug, thiserror::Error)]
    #[error("test error")]
    struct TestKind;

    impl Kind for TestKind {}

    define_error! {
        struct TestError;
    }
    define_error! {
        struct TestKindError {
            type Kind = TestKind;
        }
    }
    define_error! {
        struct TestOptKindError {
            type OptKind = TestKind;
        }
    }

    #[test]
    fn test_any_diag_wrapper() {
        let diag = AnyDiag::new().msg("something went wrong");
        let err = TestError::from(diag);
        assert_eq!(err.to_string(), "something went wrong");
    }

    #[test]
    fn test_kind_diag_wrapper() {
        let diag = Diag::new().kind(TestKind);
        let err = TestKindError::from(diag);
        assert_eq!(err.to_string(), "test error");
        assert!(matches!(err.kind(), TestKind));
    }

    #[test]
    fn test_opt_kind_diag_wrapper() {
        let diag = Diag::new().kind(TestKind);
        let err = TestOptKindError::from(diag);
        assert_eq!(err.to_string(), "test error");
        assert!(matches!(err.opt_kind(), Some(TestKind)));
    }
}
