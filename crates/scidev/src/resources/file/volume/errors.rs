use std::io;

use crate::{
    resources::{ConversionError, ResourceId},
    utils::errors::AnyInvalidDataError,
};

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error(transparent)]
    Io(io::Error),
    #[error(transparent)]
    MemReader(#[from] AnyInvalidDataError),
    #[error(transparent)]
    Conversion(#[from] ConversionError),
    #[error("Invalid resource location {location:x}: {reason}")]
    InvalidResourceLocation { location: usize, reason: String },
    #[error("Resource ID mismatch: expected {expected:?}, got {got:?}")]
    ResourceIdMismatch {
        expected: ResourceId,
        got: ResourceId,
    },
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        match err.downcast() {
            Ok(err) => err,
            Err(err) => Self::Io(err),
        }
    }
}
