use std::io;

use crate::{
    resources::ConversionError,
    utils::{block::FromBlockSourceError, errors::AnyInvalidDataError},
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    MemReader(#[from] AnyInvalidDataError),
    #[error(transparent)]
    Conversion(#[from] ConversionError),
}

impl From<FromBlockSourceError> for Error {
    fn from(err: FromBlockSourceError) -> Self {
        match err {
            FromBlockSourceError::Io(io_err) => Self::Io(io_err),
            FromBlockSourceError::MemReader(mem_err) => Self::MemReader(mem_err),
            FromBlockSourceError::Conversion(err) => Self::Conversion(ConversionError::new(err)),
        }
    }
}
