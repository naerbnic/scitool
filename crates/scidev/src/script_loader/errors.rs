use std::fmt::{Debug, Display};

use crate::utils::errors::{OtherError, OtherMapper};

#[derive(Debug, thiserror::Error)]
#[error("Malformed data: {0}")]
pub struct MalformedDataError(#[from] OtherError);

impl MalformedDataError {
    pub fn new<M>(msg: M) -> Self
    where
        M: Display + Debug + Send + Sync + 'static,
    {
        MalformedDataError(OtherError::from_msg(msg))
    }

    pub fn new_with_cause<E>(context: String, cause: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self(OtherError::new(cause).add_context(context))
    }

    pub fn map_from<I, E>(context: I) -> impl FnOnce(E) -> Self
    where
        I: Into<String>,
        E: std::error::Error + Send + Sync + 'static,
    {
        move |e| Self::new_with_cause(context.into(), e)
    }
}

pub struct MalformedData;

impl OtherMapper for MalformedData {
    type Error = MalformedDataError;

    fn map_other(self, other: OtherError) -> Self::Error {
        MalformedDataError(other)
    }
}
