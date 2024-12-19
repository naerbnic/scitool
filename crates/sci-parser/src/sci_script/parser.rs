use std::fmt::Error;

use crate::sexpr::{parse_funcs::SExprInput, SExpr};

#[expect(dead_code)]
pub fn parse_script(_input: SExprInput<'_>) -> Result<Vec<SExpr>, Error> {
    todo!()
}
