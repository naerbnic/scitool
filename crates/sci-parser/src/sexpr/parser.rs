use nom::{
    combinator::{complete, eof, map as nom_map},
    multi::many0,
    sequence::tuple,
    Finish, Parser,
};

use crate::{
    inputs::text::InputRange,
    tokens::parse_funcs::{
        lbracket, lparen, num, rbracket, rparen, string, symbol, NomError as TokenNomError,
        TokenInput, TokenParser,
    },
};

use super::{Atom, Contents, SExpr};

pub fn parse_sequence<'a>() -> impl TokenParser<'a, Vec<SExpr>> {
    many0(parse_tokens)
}

pub fn parse_list<'a>() -> impl TokenParser<'a, (Vec<SExpr>, InputRange)> {
    tuple((lparen(), many0(parse_tokens), rparen()))
        .map(|(lp, items, rp)| (items, lp.location().merge(rp.location())))
}

pub fn parse_bracket_list<'a>() -> impl TokenParser<'a, (Vec<SExpr>, InputRange)> {
    tuple((lbracket(), many0(parse_tokens), rbracket()))
        .map(|(lp, items, rp)| (items, lp.location().merge(rp.location())))
}

pub fn parse_atom<'a>() -> impl TokenParser<'a, (Atom, InputRange)> {
    nom::branch::alt((
        nom_map(symbol(), |sym| {
            (Atom::Symbol(sym.value().to_string()), sym.location())
        }),
        nom_map(string(), |sym| {
            (Atom::String(sym.value().to_string()), sym.location())
        }),
        nom_map(num(), |sym| (Atom::Number(*sym.value()), sym.location())),
    ))
}

pub fn parse_tokens(
    input: TokenInput<'_>,
) -> nom::IResult<TokenInput<'_>, SExpr, TokenNomError<'_>> {
    nom::branch::alt((
        nom_map(parse_list(), |(items, range)| {
            SExpr::new(Contents::List(items), range)
        }),
        nom_map(parse_atom(), |(atom, range)| {
            SExpr::new(Contents::Atom(atom), range)
        }),
    ))(input)
}

pub fn parse_buffer(tokens: TokenInput<'_>) -> Result<Vec<SExpr>, TokenNomError<'_>> {
    let (_, result) =
        complete(tuple((parse_sequence(), eof)).map(|(items, _)| items))(tokens).finish()?;
    Ok(result)
}
