use std::fmt::Error;

use crate::sexpr::{SExpr, parse_funcs::SExprInput};

#[expect(dead_code)]
pub fn parse_script(_input: SExprInput<'_>) -> Result<Vec<SExpr>, Error> {
    todo!()
}
