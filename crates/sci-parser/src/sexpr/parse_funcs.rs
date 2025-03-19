use nom::{
    IResult, InputLength, InputTake, Parser,
    combinator::{eof, map as nom_map, map_opt, verify},
    error::{ErrorKind, ParseError, VerboseError},
};

use crate::inputs::{slice::Input, text::InputRange};

use super::{Atom, Contents, SExpr};

pub type SExprInput<'a> = Input<'a, SExpr>;
pub type NomError<'a> = VerboseError<SExprInput<'a>>;

pub trait SExprParser<'a, T>: Parser<SExprInput<'a>, T, NomError<'a>> {}

impl<'a, T, B> SExprParser<'a, T> for B where B: Parser<SExprInput<'a>, T, NomError<'a>> {}

pub struct ParsedExpr<T> {
    value: T,
    location: InputRange,
}

impl<T> ParsedExpr<T> {
    pub fn new(value: T, location: InputRange) -> Self {
        Self { value, location }
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> ParsedExpr<U> {
        ParsedExpr::new(f(self.value), self.location)
    }

    pub fn value(&self) -> &T {
        &self.value
    }

    pub fn location(&self) -> InputRange {
        self.location
    }
}

fn next<'a, T, E>(input: Input<'a, T>) -> IResult<Input<'a, T>, &'a T, E>
where
    E: ParseError<Input<'a, T>>,
{
    if input.input_len() == 0 {
        return Err(nom::Err::Error(E::from_error_kind(input, ErrorKind::Eof)));
    }
    let item = &input.content_slice()[0];
    let next_input = input.take_split(1).0;
    Ok((next_input, item))
}

/// Parses a single symbol atom from the SExpr. Returns the symbol as a string slice,
/// along with text location information.
pub fn symbol<'a>() -> impl SExprParser<'a, ParsedExpr<&'a str>> {
    map_opt(next, |t: &SExpr| match &t.contents {
        Contents::Atom(Atom::Symbol(s)) => Some(ParsedExpr::new(s.as_str(), t.location)),
        _ => None,
    })
}

/// Parses a single string atom from the SExpr. Returns the string as a string slice,
/// along with text location information.
pub fn str<'a>() -> impl SExprParser<'a, ParsedExpr<&'a str>> {
    map_opt(next, |t: &SExpr| match &t.contents {
        Contents::Atom(Atom::String(s)) => Some(ParsedExpr::new(s.as_str(), t.location)),
        _ => None,
    })
}

/// Parses a single number atom from the SExpr. Returns the number as an i64,
/// along with text location information.
pub fn num<'a>() -> impl SExprParser<'a, ParsedExpr<i64>> {
    map_opt(next, |t: &SExpr| match &t.contents {
        Contents::Atom(Atom::Number(n)) => Some(ParsedExpr::new(*n, t.location)),
        _ => None,
    })
}

fn list_inner<'a>() -> impl SExprParser<'a, ParsedExpr<&'a [SExpr]>> {
    map_opt(next, |t: &SExpr| match &t.contents {
        Contents::List(contents) => Some(ParsedExpr::new(&contents[..], t.location)),
        _ => None,
    })
}

/// Parses a single list and its contents SExpr. The contents of the list are
/// parsed using the given parser.
///
/// The result is annotated with the text location of the list.
pub fn list<'a, P, T>(mut list_parser: P) -> impl SExprParser<'a, ParsedExpr<T>>
where
    P: SExprParser<'a, T>,
{
    move |input| {
        let (input, contents) = list_inner().parse(input)?;
        let list_input = Input::new(contents.value());
        let (list_input, result) = list_parser.parse(list_input)?;
        let _ = eof(list_input)?;
        Ok((input, ParsedExpr::new(result, contents.location())))
    }
}

/// Parses a symbol with the given literal name. Useful for parsing keywords.
pub fn symbol_lit(literal: &str) -> impl SExprParser<'_, ParsedExpr<()>> {
    nom_map(
        verify(symbol(), move |sym| *sym.value() == literal),
        |sym: ParsedExpr<_>| sym.map(|_| ()),
    )
}
