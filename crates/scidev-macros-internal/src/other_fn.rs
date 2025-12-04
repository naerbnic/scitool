use quote::quote;
use syn::ItemFn;

pub(crate) fn other_fn(
    attr: proc_macro2::TokenStream,
    item: proc_macro2::TokenStream,
) -> syn::Result<proc_macro2::TokenStream> {
    if !attr.is_empty() {
        return Err(syn::Error::new_spanned(
            attr,
            "the `other_err` attribute does not take any arguments",
        ));
    }
    let input_fn: ItemFn = syn::parse2(item)?;

    let mut item_fn: ItemFn = input_fn;

    let block = &item_fn.block;

    let result_id = syn::Ident::new("result", proc_macro2::Span::call_site());

    let new_body = quote! { {
        let #result_id: Result<_, Box<dyn std::error::Error + Send + Sync>> =
            (move || { #block })();
        match #result_id {
            Ok(val) => Ok(val),
            Err(e) => Err(e.into()),
        }
    }};

    let new_body_block: syn::Block =
        syn::parse2(new_body).expect("failed to parse new function body");

    item_fn.block = Box::new(new_body_block);

    Ok(quote! { #item_fn })
}
