use std::collections::{BTreeMap, btree_map::Entry};

use proc_macro_crate::FoundCrate;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Ident, Lifetime};

struct LabelInfo {
    source_token: Lifetime,
}

pub struct EntryState {
    crate_name: TokenStream,
    data_var: Ident,
    seen_labels: BTreeMap<String, LabelInfo>,
    loc_map_var: Ident,
    patch_ops_var: Ident,
}

impl EntryState {
    pub fn new(data_var: Ident, loc_map_var: Ident, patch_ops_var: Ident) -> Self {
        Self {
            crate_name: match proc_macro_crate::crate_name(crate::BASE_CRATE)
                .expect("crate name lookup failed")
            {
                FoundCrate::Itself => quote! { crate },
                FoundCrate::Name(crate_name) => {
                    let crate_name_ident = format_ident!("{}", crate_name);
                    quote! { ::#crate_name_ident }
                }
            },
            data_var,
            seen_labels: BTreeMap::new(),
            loc_map_var,
            patch_ops_var,
        }
    }

    pub fn report_new_label(&mut self, label: &Lifetime) -> syn::Result<()> {
        let label_str = label.ident.to_string();
        match self.seen_labels.entry(label_str) {
            Entry::Vacant(vacant) => vacant.insert(LabelInfo {
                source_token: label.clone(),
            }),
            Entry::Occupied(occ) => {
                let mut err1 = syn::Error::new_spanned(label, "Duplicate label");
                err1.combine(syn::Error::new_spanned(
                    &occ.get().source_token,
                    "Originally defined here",
                ));

                return Err(err1);
            }
        };

        Ok(())
    }

    pub fn data_var(&self) -> &Ident {
        &self.data_var
    }

    pub fn loc_map_var(&self) -> &Ident {
        &self.loc_map_var
    }

    pub fn generate_expr(&self, statements: TokenStream) -> TokenStream {
        let data_var = &self.data_var;
        let loc_map_var = &self.loc_map_var;
        let patch_ops_var = &self.patch_ops_var;
        let crate_name = &self.crate_name;
        quote! {
            {
                let mut #data_var: Vec<u8> = Vec::new();
                let mut #loc_map_var = #crate_name::support::LocationMap::new();
                let mut #patch_ops_var: Vec<#crate_name::support::PatchOp> = Vec::new();
                {#statements}
                {
                    for op in #patch_ops_var {
                        op.apply(&mut #data_var);
                    }
                }
                #data_var
            }
        }
    }

    pub fn crate_name(&self) -> &TokenStream {
        &self.crate_name
    }
}
