//! A generic error type for ergonomic error handling/reporting.

use std::borrow::Cow;
use std::fmt::Debug;
use std::fmt::Display;

pub struct OtherError(anyhow::Error);

impl Display for OtherError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Debug for OtherError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl std::error::Error for OtherError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

pub trait ResultExt<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn with_other_err(self) -> Result<T, OtherError>;
    #[expect(single_use_lifetimes, reason = "anon lifetimes in impls is unstable")]
    fn with_other_context<'a>(self, msg: impl Into<Cow<'a, str>>) -> Result<T, OtherError>;
}

impl<T, E> ResultExt<T, E> for Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn with_other_err(self) -> Result<T, OtherError> {
        self.map_err(|e| OtherError(anyhow::Error::new(e)))
    }
    #[expect(single_use_lifetimes, reason = "anon lifetimes in impls is unstable")]
    fn with_other_context<'a>(self, msg: impl Into<Cow<'a, str>>) -> Result<T, OtherError> {
        self.map_err(move |e| OtherError(anyhow::Error::new(e).context(msg.into().into_owned())))
    }
}
