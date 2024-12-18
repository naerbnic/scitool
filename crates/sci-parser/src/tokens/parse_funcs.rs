use nom::{InputLength, InputTake};

use crate::inputs::{slice::Input, text::InputRange};

use super::{Contents, Token};

pub type TokenInput<'a> = Input<'a, Token>;
pub type NomError<'a> = nom::error::VerboseError<TokenInput<'a>>;

pub trait TokenParser<'a, T>: nom::Parser<TokenInput<'a>, T, NomError<'a>> {}

impl<'a, T, B> TokenParser<'a, T> for B where B: nom::Parser<TokenInput<'a>, T, NomError<'a>> {}

pub struct TokenResult<T> {
    value: T,
    location: InputRange,
}

impl<T> TokenResult<T> {
    pub fn from_token(value: T, token: &Token) -> Self {
        Self {
            value,
            location: token.location,
        }
    }

    pub fn value(&self) -> &T {
        &self.value
    }

    pub fn location(&self) -> InputRange {
        self.location
    }
}

pub fn next<'a, T, E>(input: Input<'a, T>) -> nom::IResult<Input<'a, T>, &'a T, E>
where
    E: nom::error::ParseError<Input<'a, T>>,
{
    if input.input_len() == 0 {
        return Err(nom::Err::Error(E::from_error_kind(
            input,
            nom::error::ErrorKind::Eof,
        )));
    }
    let item = &input.content_slice()[0];
    let next_input = input.take_split(1).0;
    Ok((next_input, item))
}

pub fn lparen<'a>() -> impl TokenParser<'a, TokenResult<()>> {
    nom::combinator::map_opt(next, |t: &Token| match t.contents {
        Contents::LParen => Some(TokenResult::from_token((), t)),
        _ => None,
    })
}

pub fn rparen<'a>() -> impl TokenParser<'a, TokenResult<()>> {
    nom::combinator::map_opt(next, |t: &Token| match t.contents {
        Contents::RParen => Some(TokenResult::from_token((), t)),
        _ => None,
    })
}

pub fn num<'a>() -> impl TokenParser<'a, TokenResult<i64>> {
    nom::combinator::map_opt(next, |t: &Token| match &t.contents {
        Contents::Number(n) => Some(TokenResult::from_token(*n, t)),
        _ => None,
    })
}

pub fn symbol<'a>() -> impl TokenParser<'a, TokenResult<&'a str>> {
    nom::combinator::map_opt(next, |t: &Token| match &t.contents {
        Contents::Symbol(s) => Some(TokenResult::from_token(s.as_str(), t)),
        _ => None,
    })
}

pub fn string<'a>() -> impl TokenParser<'a, TokenResult<&'a str>> {
    nom::combinator::map_opt(next, |t: &Token| match &t.contents {
        Contents::String(s) => Some(TokenResult::from_token(s.as_str(), t)),
        _ => None,
    })
}
