use proc_macro2::{Span, TokenStream};
use quote::quote;

#[derive(Clone, Copy, Debug)]
pub enum ArgType {
    Label,
    VarUWord,
    VarSWord,
    Byte,
}

struct NamesList<T>(Vec<(syn::Ident, T)>);

impl<T> NamesList<T> {
    pub fn from_iter(prefix: &str, iter: impl IntoIterator<Item = T>) -> Self {
        NamesList(
            iter.into_iter()
                .enumerate()
                .map(|(i, item)| {
                    (
                        syn::Ident::new(&format!("{}{}", prefix, i), Span::call_site()),
                        item,
                    )
                })
                .collect(),
        )
    }

    pub fn name_iter(&self) -> Vec<&syn::Ident> {
        self.0.iter().map(|(name, _)| name).collect()
    }
}

impl ArgType {
    pub fn asm_arg_type_name(&self, label_type_var: &syn::Ident) -> TokenStream {
        match self {
            ArgType::Label => quote! { Label<#label_type_var> },
            ArgType::VarUWord => quote! { VarUWord },
            ArgType::VarSWord => quote! { VarSWord },
            ArgType::Byte => quote! { Byte },
        }
    }

    pub fn arg_type_name(&self) -> TokenStream {
        match self {
            // Labels are variable width signed words.
            ArgType::Label => quote! { VarSWord },
            ArgType::VarUWord => quote! { VarUWord },
            ArgType::VarSWord => quote! { VarSWord },
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
            "VarSWord" => Ok(ArgType::VarSWord),
            "Byte" => Ok(ArgType::Byte),
            _ => Err(syn::Error::new(ident.span(), "unknown argument type")),
        }
    }
}

fn impl_write_asm_arg_expr(
    buf_var: &syn::Ident,
    arg_var: &syn::Ident,
    arg_width_var: &syn::Ident,
    rest_args: &[&ArgType],
) -> TokenStream {
    let rest_arg_type_names = rest_args.iter().map(|ty| ty.arg_type_name());
    let rest_size_expr = quote! {
        {
            0 #(+ <#rest_arg_type_names as InstArgBase>::byte_size(#arg_width_var))*
        }
    };

    quote! {
        #arg_var.write_asm_arg(#arg_width_var, #rest_size_expr, &mut #buf_var)?;
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
    _paren: syn::token::Paren,
    opcode: OpcodeDefParsed,
    // arg_width: syn::Ident,
    _arg_types_paren: syn::token::Paren,
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
            .map(|arg_type| arg_type.asm_arg_type_name(label_type_var));
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
        let arg_types = self.arg_types.iter().map(ArgType::arg_type_name);
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

    pub fn impl_from_opcode_byte_clause(&self) -> TokenStream {
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

    pub fn impl_opcode_byte_clause(&self) -> TokenStream {
        let id = &self.id;
        match &self.opcode {
            OpcodeDefParsed::LocalDef { .. } => {
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

    pub fn impl_opcode_name_clause(&self) -> TokenStream {
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

    pub fn impl_opcode_clause(&self, enum_name: &syn::Ident) -> TokenStream {
        let id = &self.id;
        let arg_types = self.arg_types.iter().map(|_| quote! { _ });
        match &self.opcode {
            OpcodeDefParsed::LocalDef { .. } => {
                quote! {
                    #enum_name::#id(opcode, #(#arg_types),*) => PMachineOpcode::#id(opcode.clone()),
                }
            }
            OpcodeDefParsed::LiteralDef { .. } => {
                quote! {
                    #enum_name::#id(#(#arg_types),*) => PMachineOpcode::#id,
                }
            }
        }
    }

    pub fn impl_inst_byte_size_clause(&self, enum_name: &syn::Ident) -> TokenStream {
        let id = &self.id;
        let arg_wildcards = self.arg_types.iter().map(|_| quote! { _ });
        let arg_types = self.arg_types.iter().map(ArgType::arg_type_name);
        match &self.opcode {
            OpcodeDefParsed::LocalDef { .. } => {
                quote! {
                    #enum_name::#id(opcode, #(#arg_wildcards),*) => {
                        let mut byte_size = 0;
                        #(
                            byte_size += #arg_types::byte_size(arg_width);
                        )*
                        byte_size
                    }
                }
            }
            OpcodeDefParsed::LiteralDef { .. } => {
                quote! {
                    #enum_name::#id(#(#arg_wildcards),*) => {
                        let mut byte_size = 0;
                        #(
                            byte_size += #arg_types::byte_size(arg_width);
                        )*
                        byte_size
                    }
                }
            }
        }
    }

    pub fn impl_write_inst_clause(&self) -> TokenStream {
        let id = &self.id;
        let args = NamesList::from_iter("arg", self.arg_types.iter());
        let arg_names = args.name_iter();
        match &self.opcode {
            OpcodeDefParsed::LocalDef { .. } => {
                quote! {
                    PMachineInst::#id(opcode, #(#arg_names),*) => {
                        let width_bit = if let ArgsWidth::Word = arg_width { 0x00 } else { 0x01 };
                        write_byte(&mut buf, opcode.opcode_byte() | width_bit)?;
                        #(
                            #arg_names.write_arg(arg_width, &mut buf)?;
                        )*
                    }
                }
            }
            OpcodeDefParsed::LiteralDef { value, .. } => {
                let high_bits: u8 = value.base10_parse::<u8>().unwrap() << 1;
                quote! {
                    PMachineInst::#id(#(#arg_names),*) => {
                        let width_bit = if let ArgsWidth::Word = arg_width { 0x00 } else { 0x01 };
                        write_byte(&mut buf, #high_bits | width_bit)?;
                        #(
                            #arg_names.write_arg(arg_width, &mut buf)?;
                        )*
                    }
                }
            }
        }
    }

    pub fn impl_asm_write_inst_clause(&self) -> TokenStream {
        let id = &self.id;
        let arg_types_vec = self.arg_types.iter().collect::<Vec<_>>();
        let args = NamesList::from_iter("arg", self.arg_types.iter());
        let arg_names = args.name_iter();
        let arg_write_exprs = arg_names.iter().enumerate().map(|(i, arg_name)| {
            let rest_arg_types = &arg_types_vec[i..];
            impl_write_asm_arg_expr(
                &syn::Ident::new("buf", Span::call_site()),
                arg_name,
                &syn::Ident::new("arg_width", Span::call_site()),
                rest_arg_types,
            )
        });
        match &self.opcode {
            OpcodeDefParsed::LocalDef { .. } => {
                quote! {
                    PMachineAsmInst::#id(opcode, #(#arg_names),*) => {
                        let width_bit = if let ArgsWidth::Word = arg_width { 0x00 } else { 0x01 };
                        buf.write_u8(opcode.opcode_byte() | width_bit);
                        #(
                            #arg_write_exprs
                        )*
                    }
                }
            }
            OpcodeDefParsed::LiteralDef { value, .. } => {
                let high_bits: u8 = value.base10_parse::<u8>().unwrap() << 1;
                quote! {
                    PMachineAsmInst::#id(#(#arg_names),*) => {
                        let width_bit = if let ArgsWidth::Word = arg_width { 0x00 } else { 0x01 };
                        buf.write_u8(#high_bits | width_bit);
                        #(
                            #arg_write_exprs
                        )*
                    }
                }
            }
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
            _paren: paren,
            opcode,
            // arg_width,
            _arg_types_paren: arg_types_paren,
            arg_types,
        })
    }
}

pub struct InstDefListParsed {
    inst_defs: syn::punctuated::Punctuated<InstDefParsed, syn::Token![;]>,
}

impl InstDefListParsed {
    pub fn to_stream(&self) -> TokenStream {
        // Type names to be able to reuse code over multiple implementations.
        let inst_type_name = syn::Ident::new("PMachineInst", Span::call_site());
        let asm_inst_type_name = syn::Ident::new("PMachineAsmInst", Span::call_site());
        let label_type_var = syn::Ident::new("LabelT", Span::call_site());

        let opcode_enum_items = self.inst_defs.iter().map(InstDefParsed::opcode_enum_item);
        let inst_enum_items = self.inst_defs.iter().map(InstDefParsed::inst_enum_item);
        let from_opcode_byte_impl = self.impl_from_opcode_byte();
        let opcode_byte_impl = self.impl_opcode_byte();
        let opcode_name_impl = self.impl_opcode_name();
        let opcode_impl = self.impl_opcode(&inst_type_name);
        let asm_opcode_impl = self.impl_opcode(&asm_inst_type_name);
        let inst_size_impl = self.impl_inst_size(&inst_type_name);
        let asm_inst_size_impl = self.impl_inst_size(&asm_inst_type_name);
        let write_inst_impl = self.impl_write_inst();
        let asm_write_inst_impl = self.impl_asm_write_inst(&label_type_var);
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
                #inst_size_impl
            }

            impl Inst for PMachineInst {
                #write_inst_impl
            }

            #[derive(Clone, Copy, Debug)]
            pub enum PMachineAsmInst<#label_type_var> {
                #(#asm_inst_enum_items),*
            }

            impl<#label_type_var> InstBase for PMachineAsmInst<#label_type_var> {
                type Opcode = PMachineOpcode;
                #asm_inst_size_impl
                #asm_opcode_impl
            }

            impl<#label_type_var> AsmInst<#label_type_var> for PMachineAsmInst<#label_type_var> where #label_type_var: Clone{
                #asm_write_inst_impl
            }
        }
    }

    fn impl_from_opcode_byte(&self) -> TokenStream {
        let opcode_enum_items = self
            .inst_defs
            .iter()
            .map(InstDefParsed::impl_from_opcode_byte_clause);
        quote! {
            fn from_opcode_byte(opcode: u8) -> anyhow::Result<Option<Self>> {
                #(#opcode_enum_items)*
                Ok(None)
            }
        }
    }

    fn impl_opcode_byte(&self) -> TokenStream {
        let opcode_enum_items = self
            .inst_defs
            .iter()
            .map(InstDefParsed::impl_opcode_byte_clause);
        quote! {
            fn opcode_byte(&self) -> u8 {
                match self {
                    #(#opcode_enum_items)*
                }
            }
        }
    }

    fn impl_opcode_name(&self) -> TokenStream {
        let opcode_enum_items = self
            .inst_defs
            .iter()
            .map(InstDefParsed::impl_opcode_name_clause);
        quote! {
            fn opcode_name(&self) -> Cow<str> {
                match self {
                    #(#opcode_enum_items)*
                }
            }
        }
    }

    fn impl_opcode(&self, type_name: &syn::Ident) -> TokenStream {
        let opcode_enum_items = self
            .inst_defs
            .iter()
            .map(|def| def.impl_opcode_clause(type_name));
        quote! {
            fn opcode(&self) -> Self::Opcode {
                match self {
                    #(#opcode_enum_items)*
                }
            }
        }
    }

    fn impl_inst_size(&self, self_type: &syn::Ident) -> TokenStream {
        let inst_enum_items = self
            .inst_defs
            .iter()
            .map(|def| def.impl_inst_byte_size_clause(self_type));
        quote! {
            fn byte_size(&self, arg_width: ArgsWidth) -> usize {
                match self {
                    #(#inst_enum_items)*
                }
            }
        }
    }

    fn impl_write_inst(&self) -> TokenStream {
        let inst_enum_items = self
            .inst_defs
            .iter()
            .map(InstDefParsed::impl_write_inst_clause);
        quote! {
            fn write_inst<W: std::io::Write>(&self, arg_width: ArgsWidth, mut buf: W) -> anyhow::Result<()> {
                match self {
                    #(#inst_enum_items)*
                }
                Ok(())
            }
        }
    }

    fn impl_asm_write_inst(&self, label_type: &syn::Ident) -> TokenStream {
        let asm_write_inst_clauses = self
            .inst_defs
            .iter()
            .map(|def| def.impl_asm_write_inst_clause());
        quote! {
            fn write_inst<Sym: Clone, W: RelocWriter<Sym, #label_type>>(&self, arg_width: ArgsWidth, mut buf: W) -> anyhow::Result<()> {
                match self {
                    #(#asm_write_inst_clauses)*
                }
                Ok(())
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
