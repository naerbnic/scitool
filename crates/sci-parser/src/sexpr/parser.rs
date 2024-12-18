use nom::{Finish, Parser};

use crate::{
    inputs::text::InputRange,
    tokens::parse_funcs::{
        lparen, num, rparen, string, symbol, NomError as TokenNomError, TokenInput, TokenParser,
    },
};

use super::{Atom, Contents, SExpr};

pub fn parse_sequence<'a>() -> impl TokenParser<'a, Vec<SExpr>> {
    nom::multi::many0(parse_tokens)
}

pub fn parse_list<'a>() -> impl TokenParser<'a, (Vec<SExpr>, InputRange)> {
    nom::sequence::tuple((lparen(), nom::multi::many0(parse_tokens), rparen()))
        .map(|(lp, items, rp)| (items, lp.location().merge(rp.location())))
}

pub fn parse_atom<'a>() -> impl TokenParser<'a, (Atom, InputRange)> {
    nom::branch::alt((
        nom::combinator::map(symbol(), |sym| {
            (Atom::Symbol(sym.value().to_string()), sym.location())
        }),
        nom::combinator::map(string(), |sym| {
            (Atom::String(sym.value().to_string()), sym.location())
        }),
        nom::combinator::map(num(), |sym| (Atom::Number(*sym.value()), sym.location())),
    ))
}

pub fn parse_tokens(
    input: TokenInput<'_>,
) -> nom::IResult<TokenInput<'_>, SExpr, TokenNomError<'_>> {
    nom::branch::alt((
        nom::combinator::map(parse_list(), |(items, range)| {
            SExpr::new(Contents::List(items), range)
        }),
        nom::combinator::map(parse_atom(), |(atom, range)| {
            SExpr::new(Contents::Atom(atom), range)
        }),
    ))(input)
}

pub fn parse_buffer(tokens: TokenInput<'_>) -> Result<Vec<SExpr>, TokenNomError<'_>> {
    let (_, result) = nom::combinator::complete(
        nom::sequence::tuple((parse_sequence(), nom::combinator::eof)).map(|(items, _)| items),
    )(tokens)
    .finish()?;
    Ok(result)
}
