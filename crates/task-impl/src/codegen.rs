use proc_macro_crate::FoundCrate;
use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, quote_spanned};
use syn::{
    Attribute, Ident, ImplItem, ImplItemFn, ItemImpl, ItemMod, ItemStruct, Type,
    parse::Parser as _, punctuated::Punctuated, spanned::Spanned as _,
};

use crate::prelude::*;

fn runtime_crate() -> syn::Result<TokenStream> {
    let Ok(found_crate) = proc_macro_crate::crate_name("scidev-task") else {
        syn_bail!([Span::call_site()] => "Could not find crate 'scidev-task'");
    };

    let result = match found_crate {
        FoundCrate::Itself => syn::Token![crate](Span::call_site()).into_token_stream(),
        FoundCrate::Name(name) => quote::format_ident!("{}", name).into_token_stream(),
    };

    Ok(result)
}

fn extract_derive_names(attr: &Attribute) -> syn::Result<Vec<Ident>> {
    let meta = attr.meta.require_list()?.tokens.clone();
    let parser = Punctuated::<Ident, syn::Token![,]>::parse_terminated;
    let punctuated = parser.parse2(meta)?;
    let idents = punctuated.iter().cloned().collect();
    Ok(idents)
}

fn extract_all_derive_names(attrs: &[Attribute]) -> syn::Result<Vec<Ident>> {
    let derives = attrs.iter().filter(|attr| attr.path().is_ident("derive"));

    let mut names = Vec::new();

    for derive in derives {
        names.extend(extract_derive_names(derive)?);
    }

    Ok(names)
}

fn is_compatible_derive(ident: &Ident) -> bool {
    matches!(ident.to_string().as_str(), "Debug" | "Clone")
}

fn is_result_attribute(attr: &Attribute) -> bool {
    if let syn::Meta::Path(attr_path) = &attr.meta {
        attr_path.is_ident("result")
    } else {
        false
    }
}

fn has_result_attribute(attrs: &[Attribute]) -> (Vec<Attribute>, bool) {
    let mut result = Vec::new();
    let mut has_result = false;

    for attr in attrs {
        if is_result_attribute(attr) {
            has_result = true;
        } else {
            result.push(attr.clone());
        }
    }
    (result, has_result)
}

struct CachedField<'a> {
    method: &'a ImplItemFn,
    new_attributes: Vec<Attribute>,
    has_result: bool,
    return_type: &'a Type,
}

impl<'a> CachedField<'a> {
    fn new(item: &'a ImplItem) -> syn::Result<Self> {
        let ImplItem::Fn(func) = item else {
            syn_bail!(item => "Expected function")
        };

        // The signature must be `fn name(&self) -> T` for some type T.
        syn_ensure!(&func.sig.generics => func.sig.generics.where_clause.is_none() && func.sig.generics.lt_token.is_none(); "Generics are currently unsupported");
        syn_ensure!(&func.sig => func.sig.constness.is_none(); "Const functions are unsupported");
        syn_ensure!(&func.sig => func.sig.asyncness.is_none(); "Async functions are unsupported");
        syn_ensure!(&func.sig => matches!(func.sig.safety, syn::Safety::Default); "Unsafe functions are unsupported");
        syn_ensure!(&func.sig => func.sig.abi.is_none(); "Custom ABI is unsupported");
        syn_ensure!(&func.sig => func.sig.variadic.is_none(); "Variadic functions are unsupported");

        let Some(receiver) = func.sig.receiver() else {
            syn_bail!(&func.sig.inputs => "Expected `&self` receiver");
        };

        syn_ensure!(
            receiver => matches!(&receiver.kind, syn::ReceiverKind::Reference(_, _, None))
                && receiver.mutability.is_none()
                && func.sig.inputs.len() == 1;
            "Expected `&self` as the only parameter"
        );

        let syn::ReturnType::Type(_, return_type) = &func.sig.output else {
            syn_bail!(func => "Expected return type");
        };

        let (new_attributes, has_result) = has_result_attribute(&func.attrs);

        Ok(Self {
            method: func,
            has_result,
            new_attributes,
            return_type: return_type.as_ref(),
        })
    }

    fn name(&self) -> &'a Ident {
        &self.method.sig.ident
    }

    fn span(&self) -> Span {
        self.method.sig.span()
    }

    fn gen_getter(&self, priv_field_name: &Ident) -> syn::Result<ImplItemFn> {
        if self.has_result {
            self.gen_result_getter(priv_field_name)
        } else {
            self.gen_plain_getter(priv_field_name)
        }
    }

    fn gen_plain_getter(&self, priv_field_name: &Ident) -> syn::Result<ImplItemFn> {
        let field_name = self.name();
        let old_body = &self.method.block;
        let new_body = syn::parse_quote_spanned! { self.method.sig.span() => {
            self.#priv_field_name.#field_name.get_or_init(|| {
                #old_body
            })
        } };
        let old_type = self.return_type;
        let mut method = self.method.clone();
        let syn::ReturnType::Type(_, ty) = &mut method.sig.output else {
            syn_bail!([method.span()] => "Expected return type");
        };
        let ty_span = ty.span();
        *ty = syn::parse_quote_spanned! { ty_span => &#old_type };
        method.block = new_body;
        Ok(method)
    }

    fn gen_result_getter(&self, priv_field_name: &Ident) -> syn::Result<ImplItemFn> {
        // The function is declared to be an instance of Result<T, E> for some T and E.

        let local_crate = runtime_crate()?;
        let return_type = self.return_type;

        let new_return_type = syn::parse_quote_spanned! { return_type.span() =>
            #local_crate::__runtime::RefOutResult<'_, #return_type>
        };

        let field_name = self.name();
        let old_body = &self.method.block;
        let new_body = syn::parse_quote_spanned! { self.method.sig.span() => {
            let out: #new_return_type =
                self.#priv_field_name.#field_name.get_or_try_init(|| -> #return_type {
                    #old_body
                });
            out
        } };
        let mut method = self.method.clone();
        let syn::ReturnType::Type(_, ty) = &mut method.sig.output else {
            syn_bail!([method.span()] => "Expected return type");
        };
        method.attrs.clone_from(&self.new_attributes);
        *ty = new_return_type;
        method.block = new_body;
        Ok(method)
    }

    fn gen_field_type(&self) -> syn::Result<Type> {
        let local_crate = runtime_crate()?;
        let return_type = self.return_type;

        let result = if self.has_result {
            syn::parse_quote_spanned! {return_type.span() => #local_crate::__runtime::ResultOut<#return_type> }
        } else {
            return_type.clone()
        };
        Ok(result)
    }
}

pub(crate) struct TaskStruct<'a> {
    original_mod: &'a ItemMod,
    original_struct: &'a ItemStruct,
    priv_derive: Vec<Ident>,
    original_impl: &'a ItemImpl,
    fields: &'a syn::FieldsNamed,
    cached_fields: Vec<CachedField<'a>>,
}

impl<'a> TaskStruct<'a> {
    pub(crate) fn from_mod(module: &'a ItemMod) -> syn::Result<Self> {
        let mut original_struct: Option<&ItemStruct> = None;
        let mut original_impl: Option<&ItemImpl> = None;

        let Some((_, items)) = &module.content else {
            syn_bail!(module => "Module must be define in file");
        };

        for item in items {
            match &item {
                syn::Item::Impl(item_impl) => {
                    if original_impl.replace(item_impl).is_some() {
                        syn_bail!(item_impl => "Expect only one impl block");
                    }
                }
                syn::Item::Struct(item_struct) => {
                    if original_struct.replace(item_struct).is_some() {
                        syn_bail!(item_struct => "Expect only one struct definition");
                    }
                }
                _ => syn_bail!(item => "Must be struct or impl"),
            }
        }

        let Some(original_struct) = original_struct else {
            syn_bail!(module => "Missing struct definition");
        };

        let Some(original_impl) = original_impl else {
            syn_bail!(module => "Missing impl definition");
        };

        let Type::Path(ty_path) = &*original_impl.self_ty else {
            syn_bail!([original_impl.span()] => "Expected struct as impl type");
        };

        let Some(type_ident) = ty_path.path.get_ident() else {
            syn_bail!(ty_path => "Expected impl with same local name as struct");
        };

        syn_ensure!(type_ident => type_ident == &original_struct.ident; "Impl identifier must match struct identifier");

        Self::from_item_struct(module, original_struct, original_impl)
    }

    pub(crate) fn from_item_struct(
        original_mod: &'a ItemMod,
        item: &'a ItemStruct,
        impl_item: &'a ItemImpl,
    ) -> syn::Result<Self> {
        syn_ensure!(&impl_item.generics => impl_item.generics.where_clause.is_none() && impl_item.generics.lt_token.is_none(); "Generics are currently unsupported");

        let syn::Fields::Named(fields) = &item.fields else {
            syn_bail!(item.struct_token => "Struct must have named fields (non-tuple)");
        };

        // FIXME: For now, assume that the impl is for the given struct.
        let cached_fields = impl_item
            .items
            .iter()
            .map(CachedField::new)
            .collect::<syn::Result<Vec<_>>>()?;

        let priv_derive = extract_all_derive_names(&item.attrs)?
            .into_iter()
            .filter(is_compatible_derive)
            .collect::<Vec<_>>();

        Ok(Self {
            original_mod,
            original_struct: item,
            priv_derive,
            original_impl: impl_item,
            fields,
            cached_fields,
        })
    }

    pub(crate) fn struct_name(&self) -> &'a Ident {
        &self.original_struct.ident
    }

    pub(crate) fn mod_header(&self) -> TokenStream {
        let vis = &self.original_mod.vis;
        let attrs = &self.original_mod.attrs;
        let ident = &self.original_mod.ident;
        quote_spanned! { ident.span() =>
            #[allow(nonstandard_style)]
            #(#attrs)*
            #vis mod #ident
        }
    }

    pub(crate) fn gen_new_func(
        &self,
        priv_type_name: &Ident,
        priv_field_name: &Ident,
    ) -> syn::Result<TokenStream> {
        let args = self.fields.named.iter().map(|f| {
            let field_name = &f.ident;
            let ty = &f.ty;
            quote_spanned! {f.span() =>
                #field_name: #ty
            }
        });
        let curr_crate = runtime_crate()?;
        let arg_names = self.fields.named.iter().map(|f| &f.ident);
        let priv_field_names = self.cached_fields.iter().map(CachedField::name);
        let struct_vis = &self.original_struct.vis;
        Ok(quote_spanned! { self.original_struct.ident.span() =>
            #struct_vis
            fn new(#(#args),*) -> Self {
                let #priv_field_name = #priv_type_name {
                    #(#priv_field_names: #curr_crate::__runtime::TaskResultSlot::new(),)*
                };
                Self { #(#arg_names),*, #priv_field_name }
            }
        })
    }

    pub(crate) fn gen_main_struct(
        &self,
        priv_field: &Ident,
        priv_type_name: &Ident,
    ) -> TokenStream {
        let struct_name = self.struct_name();
        let vis = &self.original_struct.vis;
        let attrs = &self.original_struct.attrs;

        let new_fields = self.fields.named.iter().map(|f| {
            let attrs = &f.attrs;
            let vis = &f.vis;
            let field_name = &f.ident;
            let field_type = &f.ty;
            quote_spanned! { f.span() =>
                #(#attrs)*
                #vis #field_name: #field_type
            }
        });
        quote_spanned! { struct_name.span() =>
            #(#attrs)*
            #vis struct #struct_name {
                #(#new_fields,)*
                #[allow(non_snake_case)]
                #priv_field: #priv_type_name,
            }
        }
    }

    /// Generates the private struct, used to store all cache entries.
    pub(crate) fn gen_priv_struct(&self, struct_name: &Ident) -> syn::Result<TokenStream> {
        let curr_crate = runtime_crate()?;
        let priv_fields = self
            .cached_fields
            .iter()
            .map(|field| {
                let field_name = &field.name();
                let ty = field.gen_field_type()?;
                Ok(quote_spanned! {field.span() =>
                    #field_name: #curr_crate::__runtime::TaskResultSlot<#ty>
                })
            })
            .collect::<syn::Result<Vec<_>>>()?;

        let priv_derives = &self.priv_derive;

        Ok(quote_spanned! { self.original_struct.span() =>
            #[allow(non_camel_case_types, clippy::all)]
            #[derive(Default, #(#priv_derives),*)]
            struct #struct_name {
                #(#priv_fields,)*
            }
        })
    }

    pub(crate) fn gen_cached_fn_impl(
        &self,
        priv_type_name: &Ident,
        priv_field_name: &Ident,
    ) -> syn::Result<ItemImpl> {
        let getters = self
            .cached_fields
            .iter()
            .map(|field| Ok(ImplItem::Fn(field.gen_getter(priv_field_name)?)))
            .collect::<syn::Result<Vec<_>>>()?;

        let struct_name = self.struct_name();
        let fn_new = self.gen_new_func(priv_type_name, priv_field_name)?;

        let mut new_impl = self.original_impl.clone();
        new_impl.items.clear();
        new_impl
            .items
            .push(syn::parse_quote_spanned! { struct_name.span() => #fn_new});
        new_impl.items.extend(getters);
        Ok(new_impl)
    }
}
