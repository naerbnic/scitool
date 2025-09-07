mod entry;
mod entry_state;
mod to_bytes;

use proc_macro::TokenStream as BaseTokenStream;
use proc_macro2::{Span, TokenStream};
use quote::quote;

use crate::{entry::DataLitEntries, entry_state::EntryState};

const BASE_CRATE: &str = "datalit";

#[proc_macro]
pub fn datalit(input: BaseTokenStream) -> BaseTokenStream {
    datalit_impl(input.into())
        .unwrap_or_else(|e| {
            let errors = e.into_iter().map(syn::Error::into_compile_error);
            quote! { {#(#errors);*}}
        })
        .into()
}

fn datalit_impl(input: TokenStream) -> syn::Result<TokenStream> {
    let entries: DataLitEntries = syn::parse2(input)?;

    let data_var = syn::Ident::new("data", Span::call_site());
    let loc_map_var = syn::Ident::new("loc_map", Span::call_site());
    let patch_ops_var = syn::Ident::new("patch_ops", Span::call_site());
    let mut state = EntryState::new(data_var.clone(), loc_map_var, patch_ops_var);
    let contents = entries.into_tokens(&mut state)?;
    Ok(state.generate_expr(contents))
}
