#[derive(Debug, thiserror::Error)]
#[error("Unexpected end of input")]
pub struct UnexpectedEndOfInput;