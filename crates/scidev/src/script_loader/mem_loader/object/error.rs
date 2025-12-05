use crate::utils::{
    errors::{InvalidDataError, OpaqueError},
    mem_reader::MemReaderError,
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
    InvalidData(#[from] InvalidDataError),

    #[error(transparent)]
    Object(#[from] ObjectError),

    #[error("Unexpected error: {0}")]
    Unexpected(#[from] OpaqueError),
}

impl From<MemReaderError> for Error {
    fn from(err: MemReaderError) -> Self {
        match err {
            MemReaderError::InvalidData(invalid_data_err) => Self::InvalidData(invalid_data_err),
            // Since we specified NoError as the MemReader's error type, this arm should be unreachable.
            MemReaderError::Read(io_err) => Self::Unexpected(OpaqueError::new(io_err)),
        }
    }
}
