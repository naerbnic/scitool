//! A generic error type for ergonomic error handling/reporting.

use std::borrow::Cow;
use std::fmt::Debug;
use std::fmt::Display;

#[derive(Debug)]
pub struct ContextError<E> {
    base_error: E,
    context: Option<String>,
}

impl<E> Display for ContextError<E>
where
    E: std::error::Error,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if let Some(ctx) = &self.context {
            write!(f, "{}: {}", ctx, self.base_error)
        } else {
            Display::fmt(&self.base_error, f)
        }
    }
}

impl<E> std::error::Error for ContextError<E>
where
    E: std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.base_error)
    }
}

impl<E> From<E> for ContextError<E>
where
    E: std::error::Error,
{
    fn from(err: E) -> Self {
        ContextError {
            base_error: err,
            context: None,
        }
    }
}

pub trait ResultExt<T, E>
where
    E: std::error::Error,
{
    #[expect(single_use_lifetimes, reason = "anon lifetimes in impls is unstable")]
    fn with_context<'a>(self, ctxt: impl Into<Cow<'a, str>>) -> Result<T, ContextError<E>>;
}

impl<T, E> ResultExt<T, E> for Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    #[expect(single_use_lifetimes, reason = "anon lifetimes in impls is unstable")]
    fn with_context<'a>(self, ctxt: impl Into<Cow<'a, str>>) -> Result<T, ContextError<E>> {
        self.map_err(|e| ContextError {
            base_error: e,
            context: Some(ctxt.into().into_owned()),
        })
    }
}
