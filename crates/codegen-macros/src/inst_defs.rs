use proc_macro2::TokenStream;
use quote::quote;

#[derive(Clone, Copy, Debug)]
pub enum ArgWidth {
    /// Both Byte and Word do the same thing.
    Any,
    /// Both variants must be separately defined.
    Both,
}

#[derive(Clone, Copy, Debug)]
pub enum ArgType {
    Label,
    Int,
    Uint,
    Kernel,
    UInt8,
    PubProc,
    Class,
    Offs,
    Prop,
    Var,
    PVar,
}

impl syn::parse::Parse for ArgType {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let ident: syn::Ident = input.parse()?;
        match ident.to_string().as_str() {
            "Label" => Ok(ArgType::Label),
            "Int" => Ok(ArgType::Int),
            "UInt" => Ok(ArgType::Uint),
            "Kernel" => Ok(ArgType::Kernel),
            "UInt8" => Ok(ArgType::UInt8),
            "PubProc" => Ok(ArgType::PubProc),
            "Class" => Ok(ArgType::Class),
            "Offs" => Ok(ArgType::Offs),
            "Prop" => Ok(ArgType::Prop),
            "Var" => Ok(ArgType::Var),
            "PVar" => Ok(ArgType::PVar),
            _ => Err(syn::Error::new(ident.span(), "unknown argument type")),
        }
    }
}

pub struct InstDefParsed {
    id: syn::Ident,
    paren: syn::token::Paren,
    name: syn::LitStr,
    opcode: syn::LitInt,
    // arg_width: syn::Ident,
    arg_types_paren: syn::token::Paren,
    arg_types: syn::punctuated::Punctuated<ArgType, syn::Token![,]>,
}

impl InstDefParsed {
    /// Generates the enum item within the Opcode enum.
    pub fn opcode_enum_item(&self) -> proc_macro2::TokenStream {
        let id = &self.id;
        quote! {
            #id
        }
    }
}

impl syn::parse::Parse for InstDefParsed {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let id = input.parse()?;
        let opcode_contents;
        let paren = syn::parenthesized!(opcode_contents in input);
        let name = opcode_contents.parse()?;
        let _: syn::Token![,] = opcode_contents.parse()?;
        let opcode = opcode_contents.parse()?;
        let _: syn::Token![,] = opcode_contents.parse()?;
        // let arg_width = opcode_contents.parse()?;
        // let _: syn::Token![,] = opcode_contents.parse()?;
        let arg_types_contents;
        let arg_types_paren = syn::parenthesized!(arg_types_contents in opcode_contents);
        let arg_types = syn::punctuated::Punctuated::parse_terminated(&arg_types_contents)?;
        // ensure the opcode contents is at the end
        if !opcode_contents.is_empty() {
            return Err(opcode_contents.error("unexpected token"));
        }

        Ok(InstDefParsed {
            id,
            paren,
            name,
            opcode,
            // arg_width,
            arg_types_paren,
            arg_types,
        })
    }
}

pub struct InstDefListParsed {
    inst_defs: syn::punctuated::Punctuated<InstDefParsed, syn::Token![;]>,
}

impl InstDefListParsed {
    pub fn opcode_enum(&self) -> TokenStream{
        let inst_enum_items = self.inst_defs.iter().map(InstDefParsed::opcode_enum_item);
        quote! {
            pub enum PMachineOpcode {
                #(#inst_enum_items),*
            }
        }
    }
}

impl syn::parse::Parse for InstDefListParsed {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(InstDefListParsed {
            inst_defs: syn::punctuated::Punctuated::parse_terminated(input)?,
        })
    }
}
