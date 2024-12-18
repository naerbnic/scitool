pub mod parser;

use crate::{inputs::text::InputRange, lexer::lex};

#[derive(Debug, PartialEq, Eq)]
pub struct SExpr {
    contents: Contents,
    location: InputRange,
}

impl SExpr {
    fn new(contents: Contents, location: InputRange) -> Self {
        Self { contents, location }
    }

    pub fn contents(&self) -> &Contents {
        &self.contents
    }

    pub fn location(&self) -> &InputRange {
        &self.location
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Atom {
    Symbol(String),
    String(String),
    Number(i64),
}

#[derive(Debug, PartialEq, Eq)]
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
    let token_input = crate::tokens::parse_funcs::TokenInput::new(&tokens);
    let sexpr = parser::parse_buffer(token_input).unwrap();
    Ok(sexpr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty() {
        let sexpr = parse("").unwrap();
        assert!(sexpr.is_empty());
    }
    #[test]
    fn parse_nil() {
        let sexpr = parse("()").unwrap();
        assert_eq!(sexpr.len(), 1);
        assert_eq!(sexpr[0].contents(), &Contents::List(vec![]));
    }
}
