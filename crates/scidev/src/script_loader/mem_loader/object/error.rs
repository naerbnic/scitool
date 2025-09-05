use crate::utils::{
    errors::{AnyInvalidDataError, NoError},
    mem_reader,
};

#[derive(Debug, thiserror::Error)]
pub enum ObjectError {
    #[error("Object data has unexpected padding bytes")]
    BadObjectPadding,
    #[error(
        "Class has script but number of properties does not equal number of fields: {num_properties} properties, {num_fields} fields"
    )]
    PropertyMismatch {
        num_properties: usize,
        num_fields: usize,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    InvalidData(#[from] AnyInvalidDataError),

    #[error(transparent)]
    Object(#[from] ObjectError),
}

impl From<mem_reader::Error<NoError>> for Error {
    fn from(err: mem_reader::Error<NoError>) -> Self {
        match err {
            mem_reader::Error::InvalidData(invalid_data_err) => Self::InvalidData(invalid_data_err),
            mem_reader::Error::BaseError(err) => err.absurd(),
        }
    }
}
