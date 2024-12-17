#[derive(Clone, Debug)]
pub struct Token {
    pub(super) contents: Contents,
    pub(super) location: TokenLocation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextRange {
    start: TextOffset,
    end: TextOffset,
}

impl TextRange {
    pub fn new(start: TextOffset, end: TextOffset) -> Self {
        Self { start, end }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextOffset {
    pub(super) offset: usize,
    pub(super) line_index: usize,
    pub(super) line_char_offset: usize,
}

#[derive(Clone, Debug)]
pub struct TokenLocation {
    path: String,
    range: TextRange,
}

impl TokenLocation {
    pub fn new(path: impl Into<String>, range: TextRange) -> Self {
        Self {
            path: path.into(),
            range,
        }
    }
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
