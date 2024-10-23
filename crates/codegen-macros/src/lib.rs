extern crate proc_macro;

mod inst_defs;

use proc_macro::TokenStream;
#[proc_macro]
pub fn define_insts(contents: TokenStream) -> TokenStream {
    let inst_defs = syn::parse_macro_input!(contents as inst_defs::InstDefListParsed);
    inst_defs.opcode_enum().into()
}
