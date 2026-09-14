use crate::codegen::TaskStruct;
use quote::quote;

#[must_use]
pub fn result_to_tokens(result: syn::Result<proc_macro2::TokenStream>) -> proc_macro2::TokenStream {
    match result {
        Ok(tokens) => tokens,
        Err(err) => err.into_compile_error(),
    }
}

pub fn task_struct(
    _args: proc_macro2::TokenStream,
    input: proc_macro2::TokenStream,
) -> syn::Result<proc_macro2::TokenStream> {
    let item = syn::parse2::<syn::ItemMod>(input)?;
    let struct_repr = TaskStruct::from_mod(&item)?;

    let priv_field_name = quote::format_ident!("__{}_priv_field", struct_repr.struct_name());

    let priv_type_name = quote::format_ident!("__{}_priv_type", struct_repr.struct_name());

    let mod_header = struct_repr.mod_header();

    let new_struct = struct_repr.gen_main_struct(&priv_field_name, &priv_type_name);
    let priv_struct = struct_repr.gen_priv_struct(&priv_type_name)?;

    let cached_fn_impl = struct_repr.gen_cached_fn_impl(&priv_type_name, &priv_field_name)?;

    Ok(quote! {
        #mod_header {
            use super::*;
            #new_struct
            #priv_struct
            #[allow(clippy::all)]
            #cached_fn_impl
        }
    })
}
