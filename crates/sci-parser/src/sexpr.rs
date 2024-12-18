pub mod input;

use crate::lexer::{lex, InputRange};

pub struct SExpr {
    contents: Contents,
    location: InputRange,
}

pub enum Atom {
    Symbol(String),
    String(String),
    Number(i64),
}

pub enum Contents {
    Atom(Atom),
    List(Vec<SExpr>),
}

#[derive(Debug, thiserror::Error)]
enum InnerError {
    #[error(transparent)]
    LexerError(#[from] crate::lexer::LexerError),
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct Error(InnerError);

pub fn parse(input: &str) -> Result<Vec<SExpr>, Error> {
    let tokens = lex(input).map_err(|e| Error(InnerError::LexerError(e)))?;
    todo!()
}
