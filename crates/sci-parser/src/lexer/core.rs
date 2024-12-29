use nom::branch::alt;
use nom::bytes::complete::is_not;
use nom::character::complete::{anychar, char};
use nom::combinator::{map, map_res, value, verify};
use nom::error::context;
use nom::multi::many0;
use nom::sequence::{pair, preceded};
use nom::InputIter;
use nom::{error::FromExternalError, Err, Parser};

use crate::inputs::text::{InputOffset, InputRange, TextInput};

use crate::tokens::{Contents, Token};

type NomError<'a> = nom::error::VerboseError<TextInput<'a>>;

pub trait TextParser<'a, T>: Parser<TextInput<'a>, T, NomError<'a>> {}

impl<'a, T, B> TextParser<'a, T> for B where B: Parser<TextInput<'a>, T, NomError<'a>> {}

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

fn parse_whitespace<'a>() -> impl TextParser<'a, ()> {
    |input| {
        let (input, _) = nom::character::complete::multispace1(input)?;
        Ok((input, ()))
    }
}

fn parse_skip<'a>() -> impl TextParser<'a, ()> {
    value(
        (),
        many0(nom::branch::alt((parse_whitespace(), line_comment()))),
    )
}

fn line_comment<'a>() -> impl TextParser<'a, ()> {
    value((), pair(char(';'), is_not("\n\r")))
}

fn parse_lit<'a, F, T>(
    mut parser: F,
    content: impl Fn() -> Contents,
) -> impl TextParser<'a, Contents>
where
    F: Parser<TextInput<'a>, T, NomError<'a>>,
{
    move |input| {
        let (input, _) = parser.parse(input)?;
        Ok((input, content()))
    }
}

fn parse_range<'a, F, T>(mut parser: F) -> impl TextParser<'a, &'a str>
where
    F: TextParser<'a, T>,
{
    move |input: TextInput<'a>| {
        let start_input = input.clone();
        let (input, _) = parser.parse(input)?;
        let chars = start_input.content_slice_up_to(&input);
        Ok((input, chars))
    }
}

fn parse_symbol<'a>() -> impl TextParser<'a, Contents> {
    let mut sequence_parser = parse_range(nom::sequence::tuple((
        nom::character::complete::satisfy(is_symbol_first_char),
        many0(nom::character::complete::satisfy(is_symbol_char)),
    )));

    move |input| {
        let (input, symbol_name) = sequence_parser.parse(input)?;
        Ok((input, Contents::Symbol(symbol_name.to_string())))
    }
}

fn parse_escaped_string_char<'a>() -> impl TextParser<'a, char> {
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

fn parse_string_char<'a>() -> impl TextParser<'a, char> {
    nom::branch::alt((
        preceded(char('\\'), parse_escaped_string_char()),
        nom::character::complete::none_of("\\\"\n\r"),
    ))
}

fn parse_string<'a>() -> impl TextParser<'a, Contents> {
    |input: TextInput<'a>| {
        let (input, _) = char('"')(input)?;
        let (input, char_vec) = many0(parse_string_char())(input)?;
        let (input, _) = char('"')(input)?;
        Ok((input, Contents::String(String::from_iter(char_vec))))
    }
}

fn parse_dec_num<'a>() -> impl TextParser<'a, i64> {
    |input: TextInput<'a>| {
        let start_input = input.clone();
        let (input, _) = nom::character::complete::digit1(input)?;
        let chars = start_input.content_slice_up_to(&input);
        match chars.parse::<i64>() {
            Ok(i) => Ok((input, i)),
            Err(e) => Err(Err::Error(NomError::from_external_error(
                start_input,
                nom::error::ErrorKind::Fail,
                format!("Tried to parse invalid integer {:?}: {}", chars, e),
            ))),
        }
    }
}

fn parse_hex_chars<'a>() -> impl TextParser<'a, i64> {
    let parser = map_res(
        nom::character::complete::hex_digit1,
        |text: TextInput<'a>| {
            let text = text.content_slice();
            if !text.iter_elements().all(|c| c.is_ascii()) {
                return Err("Non-ASCII characters in hex string".to_string());
            }
            let bytes = hex::decode(text).map_err(|e| e.to_string())?;
            if bytes.len() > 4 {
                return Err("Hex string too long".to_string());
            }

            let mut u64_bytes = [0u8; 8];
            u64_bytes[8 - bytes.len()..].copy_from_slice(&bytes);
            Ok(i64::from_be_bytes(u64_bytes))
        },
    );
    parser
}

fn parse_num<'a>() -> impl TextParser<'a, Contents> {
    |input: TextInput<'a>| {
        let (input, has_neg) = nom::combinator::opt(char('-'))(input)?;
        let (input, mut val) = alt((
            context("hex", preceded(char('$'), parse_hex_chars())),
            context("dec", parse_dec_num()),
        ))(input)?;
        if has_neg.is_some() {
            val = -val;
        }
        Ok((input, Contents::Number(val)))
    }
}

fn token_content_parser<'a>() -> impl TextParser<'a, Contents> {
    |input| {
        nom::branch::alt((
            context("lparen", parse_lit(char('('), || Contents::LParen)),
            context("rparen", parse_lit(char(')'), || Contents::RParen)),
            context("lbracket", parse_lit(char('['), || Contents::LBracket)),
            context("rbracket", parse_lit(char(']'), || Contents::RBracket)),
            // Num must come before symbol, as we allow "-1" to be an integer
            // in the lexer, but "-" by itself is a symbol.
            context("num", parse_num()),
            context("symbol", parse_symbol()),
            context("string", parse_string()),
        ))(input)
    }
}

fn token_parser<'a>() -> impl TextParser<'a, Token> {
    |input: TextInput<'a>| {
        let start_offset = input.input_offset();
        let (content_end_input, contents) = token_content_parser().parse(input)?;
        let end_offset = content_end_input.input_offset();
        let (start_input, _) = parse_skip().parse(content_end_input)?;
        let location = InputRange::new(start_offset, end_offset);
        Ok((start_input, Token { contents, location }))
    }
}

fn lexer<'a>() -> impl TextParser<'a, Vec<Token>> {
    |input| {
        let (input, _) = parse_skip().parse(input)?;
        let (input, tokens) = many0(token_parser())(input)?;
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

    #[test]
    fn test_skipped() {
        let tokens = lex("  (; comment\n 1)").unwrap();
        assert_eq!(
            tokens
                .iter()
                .map(|t| t.contents.clone())
                .collect::<Vec<_>>(),
            vec![Contents::LParen, Contents::Number(1), Contents::RParen,]
        );
    }

    #[test]
    fn test_hex_num() {
        let tokens = lex("($1234)").unwrap();
        assert_eq!(
            tokens
                .iter()
                .map(|t| t.contents.clone())
                .collect::<Vec<_>>(),
            vec![
                Contents::LParen,
                Contents::Number(0x1234i64),
                Contents::RParen,
            ]
        );
    }
}
