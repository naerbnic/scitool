use nom::{error::Error, Err, Parser};

use super::{
    input::Input,
    tokens::{Contents, TextRange, Token, TokenLocation},
};

pub(super) trait TokenContentParser<'a>:
    Parser<Input<'a>, Contents, Error<Input<'a>>>
{
}

impl<'a, P> TokenContentParser<'a> for P where P: Parser<Input<'a>, Contents, Error<Input<'a>>> {}

fn is_symbol_first_char(c: char) -> bool {
    c.is_alphabetic() || c == '_' || c == '#' || c == '@' || c == '&'
}

fn is_symbol_char(c: char) -> bool {
    is_symbol_first_char(c) || c.is_numeric()
}

fn is_whitespace(c: char) -> bool {
    c.is_whitespace()
}

fn parse_whitespace<'a>() -> impl Parser<Input<'a>, (), Error<Input<'a>>> {
    |input| {
        let (input, _) = nom::character::complete::multispace0(input)?;
        Ok((input, ()))
    }
}

fn token_content_parser<'a>() -> impl Parser<Input<'a>, Contents, Error<Input<'a>>> {
    |input| {
        Err(nom::Err::Error(Error::new(
            input,
            nom::error::ErrorKind::Char,
        )))
    }
}

fn token_parser<'a>() -> impl Parser<Input<'a>, Token, Error<Input<'a>>> {
    |input: Input<'a>| {
        let start_offset = input.input_offset();
        let (content_end_input, contents) = token_content_parser().parse(input)?;
        let end_offset = content_end_input.input_offset();
        let (start_input, _) = parse_whitespace().parse(content_end_input)?;
        let location = TokenLocation::new(
            start_input.file_path(),
            TextRange::new(start_offset, end_offset),
        );
        Ok((start_input, Token { contents, location }))
    }
}

pub(super) fn lexer<'a>() -> impl Parser<Input<'a>, Vec<Token>, Error<Input<'a>>> {
    |input| {
        let (input, _) = parse_whitespace().parse(input)?;
        let (input, tokens) = nom::multi::many0(token_parser())(input)?;
        Ok((input, tokens))
    }
}
