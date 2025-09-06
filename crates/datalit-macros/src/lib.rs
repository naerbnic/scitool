mod entry;
mod to_bytes;

use proc_macro::TokenStream as BaseTokenStream;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::punctuated::Punctuated;

use crate::{
    entry::DataLitEntry,
    to_bytes::{Endianness, IntType, base10_digits_to_bytes},
};

#[proc_macro]
pub fn datalit(input: BaseTokenStream) -> BaseTokenStream {
    datalit_impl(input.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

#[derive(derive_syn_parse::Parse)]
struct DataLitInput {
    #[call(Punctuated::parse_terminated)]
    entries: Punctuated<DataLitEntry, syn::Token![,]>,
}

fn datalit_impl(input: TokenStream) -> syn::Result<TokenStream> {
    let DataLitInput { entries } = syn::parse2(input)?;

    let data_var = syn::Ident::new("data", Span::call_site());

    let mut data_statements = Vec::new();
    for entry in entries {
        data_statements.push(entry.into_tokens(&data_var)?);
    }
    Ok(quote! {
        {
            let mut #data_var: Vec<u8> = Vec::new();
            #(#data_statements)*
            #data_var
        }
    })
}
