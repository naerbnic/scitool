use nom::{error::Error, Err, Parser};

use super::{
    input::{Input, InputOffset, InputRange},
    tokens::{Contents, Token},
};

type NomError<'a> = nom::error::VerboseError<Input<'a>>;

pub(super) trait TokenContentParser<'a>: Parser<Input<'a>, Contents, NomError<'a>> {}

impl<'a, P> TokenContentParser<'a> for P where P: Parser<Input<'a>, Contents, NomError<'a>> {}

fn is_symbol_first_char(c: char) -> bool {
    c.is_alphabetic() || c == '_' || c == '#' || c == '@' || c == '&'
}

fn is_symbol_char(c: char) -> bool {
    is_symbol_first_char(c) || c.is_numeric()
}

fn is_whitespace(c: char) -> bool {
    c.is_whitespace()
}

fn parse_whitespace<'a>() -> impl Parser<Input<'a>, (), NomError<'a>> {
    |input| {
        let (input, _) = nom::character::complete::multispace0(input)?;
        Ok((input, ()))
    }
}

fn parse_lit<'a, F, T>(
    mut parser: F,
    content: impl Fn() -> Contents,
) -> impl Parser<Input<'a>, Contents, NomError<'a>>
where
    F: Parser<Input<'a>, T, NomError<'a>>,
{
    move |input| {
        let (input, _) = parser.parse(input)?;
        Ok((input, content()))
    }
}

fn token_content_parser<'a>() -> impl Parser<Input<'a>, Contents, NomError<'a>> {
    |input| {
        nom::branch::alt((
            nom::error::context(
                "lparen",
                parse_lit(nom::character::complete::char('('), || Contents::LParen),
            ),
            nom::error::context(
                "rparen",
                parse_lit(nom::character::complete::char(')'), || Contents::RParen),
            ),
        ))(input)
    }
}

fn token_parser<'a>() -> impl Parser<Input<'a>, Token, NomError<'a>> {
    |input: Input<'a>| {
        let start_offset = input.input_offset();
        let (content_end_input, contents) = token_content_parser().parse(input)?;
        let end_offset = content_end_input.input_offset();
        let (start_input, _) = parse_whitespace().parse(content_end_input)?;
        let location = InputRange::new(start_offset, end_offset);
        Ok((start_input, Token { contents, location }))
    }
}

fn lexer<'a>() -> impl Parser<Input<'a>, Vec<Token>, NomError<'a>> {
    |input| {
        let (input, _) = parse_whitespace().parse(input)?;
        let (input, tokens) = nom::multi::many0(token_parser())(input)?;
        Ok((input, tokens))
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{lex_err}")]
pub struct LexerError<'a> {
    lex_err: NomError<'a>,
}

pub fn lex(input: &str) -> Result<Vec<Token>, LexerError> {
    let input = Input::new(input);
    match lexer().parse(input) {
        Ok((_, tokens)) => Ok(tokens),
        Err(e) => {
            let err = match e {
                Err::Incomplete(_) => unreachable!("A lexer should never return Incomplete"),
                Err::Error(err) => err,
                Err::Failure(err) => err,
            };
            Err(LexerError { lex_err: err })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lex_empty() {
        let tokens = lex("").unwrap();
        assert!(tokens.is_empty());
    }

    #[test]
    fn parse_token_content_parens() {
        let input = Input::new("()");
        let (input, token) = token_content_parser().parse(input).unwrap();
        assert_eq!(token, Contents::LParen);
        assert_eq!(input.content_slice(), ")");

        let (input, token) = token_content_parser().parse(input).unwrap();
        assert_eq!(token, Contents::RParen);
        assert!(input.content_slice().is_empty());
    }

    #[test]
    fn parse_token_parens() {
        let input = Input::new("()");
        let (input, token) = token_parser().parse(input).unwrap();
        assert_eq!(token.contents, Contents::LParen);
        assert_eq!(input.content_slice(), ")");

        let (input, token) = token_parser().parse(input).unwrap();
        assert_eq!(token.contents, Contents::RParen);
        assert!(input.content_slice().is_empty());
    }

    #[test]
    fn lex_parens() {
        let tokens = lex("()((").unwrap();
        assert_eq!(
            tokens
                .iter()
                .map(|t| t.contents.clone())
                .collect::<Vec<_>>(),
            vec![
                Contents::LParen,
                Contents::RParen,
                Contents::LParen,
                Contents::LParen,
            ]
        );
    }
}
