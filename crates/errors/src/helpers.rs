//! Utilities that assist in error reporting.

use crate::{AnyDiag, Diag, Kind, MaybeDiag, Raiser, locations::SourceLoc, raiser::RaisedToDiag};

/// Internal type representing how a given error was caught, either through
/// the generic `std::error::Error` handler, or through the propagation of a
/// Diag variant.
enum CaughtError<T> {
    /// Indicates the diag type that is intended to be rethrown transparently.
    Diag(T),
    Error(Box<dyn std::error::Error + Send + Sync>),
}

impl<T> CaughtError<T> {
    fn from_std_error<E>(err: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        CaughtError::Error(Box::new(err))
    }
}

struct ErrorContext<T> {
    loc: SourceLoc,
    error: CaughtError<T>,
}

impl<T> ErrorContext<T> {
    #[track_caller]
    fn from_std_error<E>(err: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self {
            loc: SourceLoc::current(),
            error: CaughtError::from_std_error(err),
        }
    }

    #[track_caller]
    fn from_diag(diag: T) -> Self {
        Self {
            loc: SourceLoc::current(),
            error: CaughtError::Diag(diag),
        }
    }
}

/// An error type used in functions that are intended to capture multiple
/// errors.
///
/// See [`in_err_context`] for details.
pub struct AnyDiagErrorCatcher {
    err: ErrorContext<AnyDiag>,
}

impl<E> From<E> for AnyDiagErrorCatcher
where
    E: std::error::Error + Send + Sync + 'static,
{
    #[track_caller]
    fn from(value: E) -> Self {
        AnyDiagErrorCatcher {
            err: ErrorContext::from_std_error(value),
        }
    }
}

impl From<AnyDiag> for AnyDiagErrorCatcher {
    #[track_caller]
    fn from(value: AnyDiag) -> Self {
        AnyDiagErrorCatcher {
            err: ErrorContext::from_diag(value),
        }
    }
}

impl<K> From<Diag<K>> for AnyDiagErrorCatcher
where
    K: Kind,
{
    #[track_caller]
    fn from(value: Diag<K>) -> Self {
        AnyDiagErrorCatcher {
            err: ErrorContext::from_diag(value.into()),
        }
    }
}

impl<K> From<MaybeDiag<K>> for AnyDiagErrorCatcher
where
    K: Kind,
{
    #[track_caller]
    fn from(value: MaybeDiag<K>) -> Self {
        AnyDiagErrorCatcher {
            err: ErrorContext::from_diag(value.into()),
        }
    }
}

pub fn in_err_context<T>(
    f: impl FnOnce() -> Result<T, AnyDiagErrorCatcher>,
) -> ErrorContextBinder<T, AnyDiag> {
    let result = f();

    ErrorContextBinder {
        result: result.map_err(|e| e.err),
    }
}

/// The result of an [`in_err_context`] call, used to fluently process the
/// result of an error block.
///
/// See [`in_err_context`] for more details.
pub struct ErrorContextBinder<T, E> {
    result: Result<T, ErrorContext<E>>,
}

impl<T, E> ErrorContextBinder<T, E> {
    /// If the error thrown from the context was a Diag variant, propagates that
    /// variant. If it is a `std::error::Error`, raises the error using the
    /// attached mapping function.
    pub fn map_raise_err<F, R>(self, body: F) -> Result<T, E>
    where
        F: FnOnce(&dyn std::error::Error, Raiser) -> R,
        R: RaisedToDiag<Diag = E>,
    {
        let err = match self.result {
            Ok(ok) => return Ok(ok),
            Err(err) => err,
        };
        let raiser = Raiser::new();
        let err = match err.error {
            // A value that was intended to be caught directly, and doesn't
            // need further updating.
            CaughtError::Diag(diag) => return Err(diag),
            CaughtError::Error(caught_error) => {
                let result = body(caught_error.as_ref(), raiser);
                result.into_diag([AnyDiag::from_boxed_std_error_with_loc(
                    caught_error,
                    err.loc,
                )])
            }
        };

        Err(err)
    }

    /// If the underlying error is convertible to the Diag type `E`, then
    /// simply propagate that error. For other cases, convert the error directly
    /// into a Diag, and return that as the error case of the result.
    pub fn reraise(self) -> Result<T, E>
    where
        E: From<AnyDiag>,
    {
        match self.result {
            Ok(ok) => Ok(ok),
            Err(err) => Err(match err.error {
                CaughtError::Diag(diag) => diag,
                CaughtError::Error(error) => {
                    E::from(AnyDiag::from_boxed_std_error_with_loc(error, err.loc))
                }
            }),
        }
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

    #[test]
    fn can_bail_out_of_err_fn() {
        // Bailing escapes with an error of the bailed message, rather than
        // the wrapped error.
        let result: Result<(), AnyDiag> = in_err_context(|| {
            bail!("TestError");
        })
        .map_raise_err(diag!(|_e| "General Error"));
        assert_eq!(&*format!("{}", result.unwrap_err()), "TestError");
    }

    #[test]
    fn can_raise_different_error_types() {
        // Bailing escapes with an error of the bailed message, rather than
        // the wrapped error.
        let err_generating_fn = |flag| {
            let result: Result<(), AnyDiag> = in_err_context(|| {
                if flag {
                    Err(ErrorTypeOne.into())
                } else {
                    Err(ErrorTypeTwo.into())
                }
            })
            .map_raise_err(diag!(|e| "General Error: {e}"));
            result.unwrap_err()
        };
        assert_eq!(
            &*format!("{}", err_generating_fn(true)),
            "General Error: Error 1"
        );
        assert_eq!(
            &*format!("{}", err_generating_fn(false)),
            "General Error: Error 2"
        );
    }
}
