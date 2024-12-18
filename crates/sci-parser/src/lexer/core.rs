use nom::{error::FromExternalError, Err, Parser};

use crate::inputs::text::{InputOffset, InputRange, TextInput};

use super::tokens::{Contents, Token};

type NomError<'a> = nom::error::VerboseError<TextInput<'a>>;

fn is_symbol_first_char(c: char) -> bool {
    match c {
        '_' | '#' | '@' | '&' | '-' | ':' | '+' | '*' | '/' => true,
        c if c.is_alphabetic() => true,
        _ => false,
    }
}

fn is_symbol_char(c: char) -> bool {
    is_symbol_first_char(c) || c.is_numeric()
}

fn parse_whitespace<'a>() -> impl Parser<TextInput<'a>, (), NomError<'a>> {
    |input| {
        let (input, _) = nom::character::complete::multispace0(input)?;
        Ok((input, ()))
    }
}

fn parse_lit<'a, F, T>(
    mut parser: F,
    content: impl Fn() -> Contents,
) -> impl Parser<TextInput<'a>, Contents, NomError<'a>>
where
    F: Parser<TextInput<'a>, T, NomError<'a>>,
{
    move |input| {
        let (input, _) = parser.parse(input)?;
        Ok((input, content()))
    }
}

fn parse_range<'a, F, T>(mut parser: F) -> impl Parser<TextInput<'a>, &'a str, NomError<'a>>
where
    F: Parser<TextInput<'a>, T, NomError<'a>>,
{
    move |input: TextInput<'a>| {
        let start_input = input.clone();
        let (input, _) = parser.parse(input)?;
        let chars = start_input.content_slice_up_to(&input);
        Ok((input, chars))
    }
}

fn parse_symbol<'a>() -> impl Parser<TextInput<'a>, Contents, NomError<'a>> {
    let mut sequence_parser = parse_range(nom::sequence::tuple((
        nom::character::complete::satisfy(is_symbol_first_char),
        nom::multi::many0(nom::character::complete::satisfy(is_symbol_char)),
    )));

    move |input| {
        let (input, symbol_name) = sequence_parser.parse(input)?;
        Ok((input, Contents::Symbol(symbol_name.to_string())))
    }
}

fn parse_escaped_string_char<'a>() -> impl Parser<TextInput<'a>, char, NomError<'a>> {
    use nom::character::complete::char;
    use nom::combinator::value;
    nom::branch::alt((
        value('\n', char('n')),
        value('\r', char('r')),
        value('\t', char('t')),
        value('\\', char('\\')),
        value('"', char('"')),
    ))
}

fn parse_string_char<'a>() -> impl Parser<TextInput<'a>, char, NomError<'a>> {
    nom::branch::alt((
        nom::sequence::preceded(
            nom::character::complete::char('\\'),
            parse_escaped_string_char(),
        ),
        nom::character::complete::none_of("\\\"\n\r"),
    ))
}

fn parse_string<'a>() -> impl Parser<TextInput<'a>, Contents, NomError<'a>> {
    |input: TextInput<'a>| {
        let (input, _) = nom::character::complete::char('"')(input)?;
        let (input, char_vec) = nom::multi::many0(parse_string_char())(input)?;
        let (input, _) = nom::character::complete::char('"')(input)?;
        Ok((input, Contents::String(String::from_iter(char_vec))))
    }
}

fn parse_num<'a>() -> impl Parser<TextInput<'a>, Contents, NomError<'a>> {
    |input: TextInput<'a>| {
        let start_input = input.clone();
        let (input, _) = nom::combinator::opt(nom::character::complete::char('-'))(input)?;
        let (input, _) = nom::character::complete::digit1(input)?;
        let chars = start_input.content_slice_up_to(&input);
        match chars.parse::<i64>() {
            Ok(i) => Ok((input, Contents::Number(i))),
            Err(e) => Err(Err::Error(NomError::from_external_error(
                start_input,
                nom::error::ErrorKind::Fail,
                format!("Tried to parse invalid integer {:?}: {}", chars, e),
            ))),
        }
    }
}

fn token_content_parser<'a>() -> impl Parser<TextInput<'a>, Contents, NomError<'a>> {
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
            // Num must come before symbol, as we allow "-1" to be an integer
            // in the lexer, but "-" by itself is a symbol.
            nom::error::context("num", parse_num()),
            nom::error::context("symbol", parse_symbol()),
            nom::error::context("string", parse_string()),
        ))(input)
    }
}

fn token_parser<'a>() -> impl Parser<TextInput<'a>, Token, NomError<'a>> {
    |input: TextInput<'a>| {
        let start_offset = input.input_offset();
        let (content_end_input, contents) = token_content_parser().parse(input)?;
        let end_offset = content_end_input.input_offset();
        let (start_input, _) = parse_whitespace().parse(content_end_input)?;
        let location = InputRange::new(start_offset, end_offset);
        Ok((start_input, Token { contents, location }))
    }
}

fn lexer<'a>() -> impl Parser<TextInput<'a>, Vec<Token>, NomError<'a>> {
    |input| {
        let (input, _) = parse_whitespace().parse(input)?;
        let (input, tokens) = nom::multi::many0(token_parser())(input)?;
        Ok((input, tokens))
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Lex error: {message} location: {location:?}")]
pub struct LexerError {
    message: String,
    kind: nom::error::VerboseErrorKind,
    location: InputOffset,
}

pub fn lex(input: &str) -> Result<Vec<Token>, LexerError> {
    let input = TextInput::new(input);
    match lexer().parse(input) {
        Ok((_, tokens)) => Ok(tokens),
        Err(e) => {
            let err = match e {
                Err::Incomplete(_) => unreachable!("A lexer should never return Incomplete"),
                Err::Error(err) => err,
                Err::Failure(err) => err,
            };
            Err(LexerError {
                message: err.to_string(),
                kind: err.errors.last().unwrap().1.clone(),
                location: err.errors.last().unwrap().0.input_offset(),
            })
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
        let input = TextInput::new("()");
        let (input, token) = token_content_parser().parse(input).unwrap();
        assert_eq!(token, Contents::LParen);
        assert_eq!(input.content_slice(), ")");

        let (input, token) = token_content_parser().parse(input).unwrap();
        assert_eq!(token, Contents::RParen);
        assert!(input.content_slice().is_empty());
    }

    #[test]
    fn parse_token_parens() {
        let input = TextInput::new("()");
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
    #[test]
    fn lex_symbols() {
        let tokens = lex("foo bar - -abc- tag:").unwrap();
        assert_eq!(
            tokens
                .iter()
                .map(|t| t.contents.clone())
                .collect::<Vec<_>>(),
            vec![
                Contents::Symbol("foo".to_string()),
                Contents::Symbol("bar".to_string()),
                Contents::Symbol("-".to_string()),
                Contents::Symbol("-abc-".to_string()),
                Contents::Symbol("tag:".to_string()),
            ]
        );
    }
    #[test]
    fn lex_numbers() {
        let tokens = lex("123 -456 0").unwrap();
        assert_eq!(
            tokens
                .iter()
                .map(|t| t.contents.clone())
                .collect::<Vec<_>>(),
            vec![
                Contents::Number(123),
                Contents::Number(-456),
                Contents::Number(0),
            ]
        );
    }
    #[test]
    fn lex_strings() {
        let tokens = lex(r#""foo" "bar\n" "baz""#).unwrap();
        assert_eq!(
            tokens
                .iter()
                .map(|t| t.contents.clone())
                .collect::<Vec<_>>(),
            vec![
                Contents::String("foo".to_string()),
                Contents::String("bar\n".to_string()),
                Contents::String("baz".to_string()),
            ]
        );
    }
}
