//! Utilities that assist in error reporting.

use crate::{AnyDiag, Diag, Kind, MaybeDiag, causes::Cause};

enum CaughtError<T> {
    /// Indicates the diag type that is intended to be rethrown transparently.
    Diag(T),
    /// Indicates a type that should be used as a cause, but must have another
    /// error put on top of it.
    Cause(Cause),
    Error(Box<dyn std::error::Error + Send + Sync>),
}

/// An error type used in functions that are intended to capture multiple
/// errors.
pub struct AnyDiagErrorCatcher {
    err: CaughtError<AnyDiag>,
}

impl<E> From<E> for AnyDiagErrorCatcher
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn from(value: E) -> Self {
        AnyDiagErrorCatcher {
            err: CaughtError::Error(Box::new(value)),
        }
    }
}

impl From<AnyDiag> for AnyDiagErrorCatcher {
    fn from(value: AnyDiag) -> Self {
        AnyDiagErrorCatcher {
            err: CaughtError::Diag(value),
        }
    }
}

impl<K> From<Diag<K>> for AnyDiagErrorCatcher
where
    K: Kind,
{
    fn from(value: Diag<K>) -> Self {
        AnyDiagErrorCatcher {
            err: CaughtError::Diag(value.into()),
        }
    }
}

impl<K> From<MaybeDiag<K>> for AnyDiagErrorCatcher
where
    K: Kind,
{
    fn from(value: MaybeDiag<K>) -> Self {
        AnyDiagErrorCatcher {
            err: CaughtError::Diag(value.into()),
        }
    }
}

pub struct DiagErrorCatcher<K>
where
    K: Kind,
{
    err: CaughtError<Diag<K>>,
}

impl<K, E> From<E> for DiagErrorCatcher<K>
where
    K: Kind,
    E: std::error::Error + Send + Sync + 'static,
{
    fn from(value: E) -> Self {
        DiagErrorCatcher {
            err: CaughtError::Error(Box::new(value)),
        }
    }
}

// impl<K> From<AnyDiag> for DiagErrorCatcher<K>
// where
//     K: Kind,
// {
//     fn from(value: AnyDiag) -> Self {
//         DiagErrorCatcher {
//             err: CaughtError::Diag(value),
//         }
//     }
// }

impl<K> From<Diag<K>> for DiagErrorCatcher<K>
where
    K: Kind,
{
    fn from(value: Diag<K>) -> Self {
        DiagErrorCatcher {
            err: CaughtError::Diag(value),
        }
    }
}

// pub fn in_err_context<T>(f: impl FnOnce() -> Result<T, AnyDiagErrorCatcher>) -> T {
//     f()
// }

// pub struct ErrorContextBinder<T> {}
