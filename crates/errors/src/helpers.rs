//! Utilities that assist in error reporting.

use crate::{AnyDiag, Diag, Kind, MaybeDiag, Raiser, raiser::RaisedToDiag};

enum CaughtError<T> {
    /// Indicates the diag type that is intended to be rethrown transparently.
    Diag(T),
    /// Indicates an error that was able to be unwrapped as an `AnyDiag`.
    UnwrappedErr(AnyDiag),
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

pub fn in_err_context<T>(
    f: impl FnOnce() -> Result<T, AnyDiagErrorCatcher>,
) -> ErrorContextBinder<T> {
    let result = f();

    ErrorContextBinder {
        result: result.map_err(|e| e.err),
    }
}

pub struct ErrorContextBinder<T> {
    result: Result<T, CaughtError<AnyDiag>>,
}

impl<T> ErrorContextBinder<T> {
    pub fn or_raise_err_with<F, R>(self, body: F) -> Result<T, R::Diag>
    where
        F: FnOnce(&dyn std::error::Error, Raiser) -> R,
        R: RaisedToDiag,
    {
        let err = match self.result {
            Ok(ok) => return Ok(ok),
            Err(err) => err,
        };
        let raiser = Raiser::new();
        todo!()
        // let err = match err {
        //     CaughtError::Diag(diag) => diag.into(),
        //     CaughtError::UnwrappedErr(diag) => diag.into(),
        //     CaughtError::Error(err) => {
        //         todo!()
        //     }
        // };
        // Err(err)
    }
}

#[cfg(test)]
mod tests {
    use crate::{bail, diag};

    use super::*;

    #[derive(Debug, thiserror::Error)]
    #[error("Error 1")]
    struct ErrorTypeOne;

    #[derive(Debug, thiserror::Error)]
    #[error("Error 2")]
    struct ErrorTypeTwo;

    // #[test]
    // fn can_bail_out_of_err_fn() {
    //     let result: Result<(), AnyDiag> = in_err_context(|| {
    //         bail!("TestError");
    //     })
    //     .or_raise_err_with(diag!(|e| "General Error"));
    // }
}
