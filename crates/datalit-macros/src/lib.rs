mod entry;
mod to_bytes;

use proc_macro::TokenStream as BaseTokenStream;
use proc_macro2::TokenStream;

use crate::{
    entry::DataLitEntries,
    to_bytes::{Endianness, IntType, base10_digits_to_bytes},
};

#[proc_macro]
pub fn datalit(input: BaseTokenStream) -> BaseTokenStream {
    datalit_impl(input.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

fn datalit_impl(input: TokenStream) -> syn::Result<TokenStream> {
    let entries: DataLitEntries = syn::parse2(input)?;
    entries.into_tokens()
}
