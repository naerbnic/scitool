use crate::inputs::text::InputRange;

#[derive(Clone, Debug)]
pub struct Token {
    pub(super) contents: Contents,
    pub(super) location: InputRange,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Contents {
    // Token '('
    LParen,
    // Token ')'
    RParen,
    // A symbol. This inclues identifiers, keywords, and operators.
    //
    // Examples:
    // - "foo"
    // - "instance"
    // - "method:"
    // - "&rest"
    // - "+"
    Symbol(String),
    // A literal string
    String(String),
    // A literal integer
    Number(i64),
}
