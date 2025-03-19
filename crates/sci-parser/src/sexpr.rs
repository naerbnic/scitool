pub mod parse_funcs;
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

    pub fn new_list(items: impl IntoIterator<Item = SExpr>) -> Self {
        Self::new(
            Contents::List(items.into_iter().collect()),
            InputRange::new_empty(),
        )
    }

    pub fn new_str(str: impl Into<String>) -> Self {
        Self::new(
            Contents::Atom(Atom::String(str.into())),
            InputRange::new_empty(),
        )
    }

    pub fn new_sym(sym: impl Into<String>) -> Self {
        Self::new(
            Contents::Atom(Atom::Symbol(sym.into())),
            InputRange::new_empty(),
        )
    }

    pub fn new_num(num: i64) -> Self {
        Self::new(Contents::Atom(Atom::Number(num)), InputRange::new_empty())
    }

    pub fn contents(&self) -> &Contents {
        &self.contents
    }

    pub fn location(&self) -> &InputRange {
        &self.location
    }

    pub fn structural_eq(&self, other: &Self) -> bool {
        self.contents.structural_eq(&other.contents)
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
    BracketList(Vec<SExpr>),
}

impl Contents {
    pub fn structural_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Atom(a), Self::Atom(b)) => a == b,
            (Self::List(a), Self::List(b)) => {
                if a.len() != b.len() {
                    return false;
                }
                a.iter().zip(b.iter()).all(|(a, b)| a.structural_eq(b))
            }
            _ => false,
        }
    }
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
        assert!(sexpr[0].structural_eq(&SExpr::new_list(vec![])));
    }

    #[test]
    fn parse_int_list() {
        let sexpr = parse("(1 2 3 -2)").unwrap();
        assert_eq!(sexpr.len(), 1);
        assert!(sexpr[0].structural_eq(&SExpr::new_list(vec![
            SExpr::new_num(1),
            SExpr::new_num(2),
            SExpr::new_num(3),
            SExpr::new_num(-2),
        ])));
    }

    #[test]
    fn parse_multiple_lists() {
        let sexpr = parse("(1 2) (3 4)").unwrap();
        assert_eq!(sexpr.len(), 2);
        assert!(
            sexpr[0].structural_eq(&SExpr::new_list(
                vec![SExpr::new_num(1), SExpr::new_num(2),]
            ))
        );

        assert!(
            sexpr[1].structural_eq(&SExpr::new_list(
                vec![SExpr::new_num(3), SExpr::new_num(4),]
            ))
        );
    }
}
