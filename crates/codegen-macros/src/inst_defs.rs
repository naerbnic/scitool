use proc_macro2::TokenStream;
use quote::quote;

#[derive(Clone, Copy, Debug)]
pub enum ArgType {
    Label,
    VarUWord,
    Byte,
}

impl ArgType {
    pub fn asm_arg_name(&self, label_type_var: &syn::Ident) -> TokenStream {
        match self {
            ArgType::Label => quote! { Label<#label_type_var> },
            ArgType::VarUWord => quote! { VarUWord },
            ArgType::Byte => quote! { Byte },
        }
    }

    pub fn arg_name(&self) -> TokenStream {
        match self {
            // Labels are variable width signed words.
            ArgType::Label => quote! { VarSWord },
            ArgType::VarUWord => quote! { VarUWord },
            ArgType::Byte => quote! { Byte },
        }
    }
}

impl syn::parse::Parse for ArgType {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let ident: syn::Ident = input.parse()?;
        match ident.to_string().as_str() {
            "Label" => Ok(ArgType::Label),
            "VarUWord" => Ok(ArgType::VarUWord),
            "Byte" => Ok(ArgType::Byte),
            _ => Err(syn::Error::new(ident.span(), "unknown argument type")),
        }
    }
}

pub enum OpcodeDefParsed {
    LocalDef {
        type_name: syn::Ident,
    },
    LiteralDef {
        name: syn::LitStr,
        value: syn::LitInt,
    },
}

impl syn::parse::Parse for OpcodeDefParsed {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(syn::Ident) {
            let type_name = input.parse()?;
            Ok(OpcodeDefParsed::LocalDef { type_name })
        } else if lookahead.peek(syn::LitStr) {
            let name = input.parse()?;
            let _: syn::Token![,] = input.parse()?;
            let value = input.parse()?;
            Ok(OpcodeDefParsed::LiteralDef { name, value })
        } else {
            Err(lookahead.error())
        }
    }
}

pub struct InstDefParsed {
    id: syn::Ident,
    paren: syn::token::Paren,
    opcode: OpcodeDefParsed,
    // arg_width: syn::Ident,
    arg_types_paren: syn::token::Paren,
    arg_types: syn::punctuated::Punctuated<ArgType, syn::Token![,]>,
}

impl InstDefParsed {
    /// Generates the enum item within the Opcode enum.
    pub fn opcode_enum_item(&self) -> TokenStream {
        let id = &self.id;
        match &self.opcode {
            OpcodeDefParsed::LocalDef { type_name } => {
                // A locally defined opcode takes the opcode type as an argument.
                quote! {
                    #id(#type_name)
                }
            }
            OpcodeDefParsed::LiteralDef { .. } => {
                quote! {
                    #id
                }
            }
        }
    }

    pub fn asm_inst_enum_item(&self, label_type_var: &syn::Ident) -> TokenStream {
        let id = &self.id;
        let asm_args = self
            .arg_types
            .iter()
            .map(|arg_type| arg_type.asm_arg_name(label_type_var));
        match &self.opcode {
            OpcodeDefParsed::LocalDef { type_name } => {
                // A locally defined opcode takes the opcode type as an argument.
                quote! {
                    #id(#type_name, #(#asm_args),*)
                }
            }
            OpcodeDefParsed::LiteralDef { .. } => {
                quote! {
                    #id(#(#asm_args),*)
                }
            }
        }
    }

    pub fn inst_enum_item(&self) -> TokenStream {
        let id = &self.id;
        let arg_types = self.arg_types.iter().map(ArgType::arg_name);
        match &self.opcode {
            OpcodeDefParsed::LocalDef { type_name } => {
                // A locally defined opcode takes the opcode type as an argument.
                quote! {
                    #id(#type_name, #(#arg_types),*)
                }
            }
            OpcodeDefParsed::LiteralDef { .. } => {
                quote! {
                    #id(#(#arg_types),*)
                }
            }
        }
    }

    pub fn from_opcode_byte_clause(&self) -> TokenStream {
        let id = &self.id;
        match &self.opcode {
            OpcodeDefParsed::LocalDef { type_name } => {
                quote! {
                    if let Some(opcode) = #type_name::from_opcode_byte(opcode)? {
                        return Ok(Some(PMachineOpcode::#id(opcode)));
                    }
                }
            }
            OpcodeDefParsed::LiteralDef { value, .. } => {
                let high_bits = value.base10_parse::<u8>().unwrap() << 1;
                quote! { 
                    if opcode & 0b11111110 == #high_bits {
                        return Ok(Some(PMachineOpcode::#id));
                    }
                }
            }
        }
    }

    pub fn opcode_byte_clause(&self) -> TokenStream {
        let id = &self.id;
        match &self.opcode {
            OpcodeDefParsed::LocalDef { type_name } => {
                quote! {
                    PMachineOpcode::#id(opcode) => opcode.opcode_byte(),
                }
            }
            OpcodeDefParsed::LiteralDef { value, .. } => {
                let high_bits = value.base10_parse::<u8>().unwrap() << 1;
                quote! {
                    PMachineOpcode::#id => #high_bits,
                }
            }
        }
    }

    pub fn opcode_name_clause(&self) -> TokenStream {
        let id = &self.id;
        match &self.opcode {
            OpcodeDefParsed::LocalDef { .. } => {
                quote! {
                    PMachineOpcode::#id(opcode) => opcode.opcode_name(),
                }
            }
            OpcodeDefParsed::LiteralDef { name, .. } => {
                let name = name.value();
                quote! {
                    PMachineOpcode::#id => (#name.into()),
                }
            }
        }
    }

    pub fn opcode_clause(&self) -> TokenStream {
        let id = &self.id;
        let arg_types = self.arg_types.iter().map(|_| quote! { _ });
        match &self.opcode {
            OpcodeDefParsed::LocalDef { .. } => {
                quote! {
                    PMachineInst::#id(opcode, #(#arg_types),*) => PMachineOpcode::#id(opcode.clone()),
                }
            }
            OpcodeDefParsed::LiteralDef { .. } => {
                quote! {
                    PMachineInst::#id(#(#arg_types),*) => PMachineOpcode::#id,
                }
            }
        }
    }

    pub fn write_inst_clause(&self) -> TokenStream {
        let id = &self.id;
        let arg_types = self.arg_types.iter().map(ArgType::arg_name);
        quote! {
            PMachineInst::#id(#(#arg_types),*) => todo!(),
        }
    }
}

impl syn::parse::Parse for InstDefParsed {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let id = input.parse()?;
        let opcode_contents;
        let paren = syn::parenthesized!(opcode_contents in input);
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
    pub fn to_stream(&self) -> TokenStream {
        let opcode_enum_items = self.inst_defs.iter().map(InstDefParsed::opcode_enum_item);
        let label_type_var = syn::Ident::new("LabelT", proc_macro2::Span::call_site());
        let inst_enum_items = self.inst_defs.iter().map(InstDefParsed::inst_enum_item);
        let from_opcode_byte_impl = self.impl_from_opcode_byte();
        let opcode_byte_impl = self.impl_opcode_byte();
        let opcode_name_impl = self.impl_opcode_name();
        let opcode_impl = self.impl_opcode();
        let asm_inst_enum_items = self
            .inst_defs
            .iter()
            .map(|inst| inst.asm_inst_enum_item(&label_type_var));
        quote! {
            #[derive(Clone, Copy, Debug)]
            pub enum PMachineOpcode {
                #(#opcode_enum_items),*
            }

            impl Opcode for PMachineOpcode {
                #from_opcode_byte_impl
                #opcode_byte_impl
                #opcode_name_impl
            }

            #[derive(Clone, Copy, Debug)]
            pub enum PMachineInst {
                #(#inst_enum_items),*
            }

            impl InstBase for PMachineInst {
                type Opcode = PMachineOpcode;
                #opcode_impl
            }

            // impl Inst for PMachineInst {
            //     fn write_inst<W: std::io::Write>(&self, arg_width: ArgsWidth, buf: W) -> anyhow::Result<()>;
            // }

            #[derive(Clone, Copy, Debug)]
            pub enum PMachineAsmInst<#label_type_var> {
                #(#asm_inst_enum_items),*
            }
        }
    }

    fn impl_from_opcode_byte(&self) -> TokenStream {
        let opcode_enum_items = self.inst_defs.iter().map(InstDefParsed::from_opcode_byte_clause);
        quote! {
            fn from_opcode_byte(opcode: u8) -> anyhow::Result<Option<Self>> {
                #(#opcode_enum_items)*
                Ok(None)
            }
        }
    }

    fn impl_opcode_byte(&self) -> TokenStream {
        let opcode_enum_items = self.inst_defs.iter().map(InstDefParsed::opcode_byte_clause);
        quote! {
            fn opcode_byte(&self) -> u8 {
                match self {
                    #(#opcode_enum_items)*
                }
            }
        }
    }

    fn impl_opcode_name(&self) -> TokenStream {
        let opcode_enum_items = self.inst_defs.iter().map(InstDefParsed::opcode_name_clause);
        quote! {
            fn opcode_name(&self) -> Cow<str> {
                match self {
                    #(#opcode_enum_items)*
                }
            }
        }
    }

    fn impl_opcode(&self) -> TokenStream {
        let opcode_enum_items = self.inst_defs.iter().map(InstDefParsed::opcode_clause);
        quote! {
            fn opcode(&self) -> Self::Opcode {
                match self {
                    #(#opcode_enum_items)*
                }
            }
        }
    }

    fn impl_write_inst(&self) -> TokenStream {
        let inst_enum_items = self.inst_defs.iter().map(InstDefParsed::inst_enum_item);
        quote! {
            fn write_inst<W: std::io::Write>(&self, arg_width: ArgsWidth, buf: W) -> anyhow::Result<()> {
                match self {
                    #(#inst_enum_items)*
                }
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
