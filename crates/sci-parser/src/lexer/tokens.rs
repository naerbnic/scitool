use super::input::location::TextRange;

#[derive(Clone, Debug)]
pub struct Token {
    pub(super) contents: Contents,
    pub(super) location: TextRange,
}

#[derive(Clone, Debug)]
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
    Number(u32),
}
