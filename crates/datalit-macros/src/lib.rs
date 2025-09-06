use proc_macro::TokenStream as BaseTokenStream;
use proc_macro2::{Literal, Span, TokenStream};
use quote::quote;
use syn::{LitByte, LitByteStr, LitInt, punctuated::Punctuated};

#[derive(Debug, Clone, Copy)]
enum Endianness {
    Little,
    Big,
    Native,
}

trait ToBytes {
    fn to_bytes(&self, endianness: Endianness) -> Vec<u8>;
}

macro_rules! impl_to_bytes_for_primitive {
    ($($t:ty),*) => {
        $(
            impl ToBytes for $t {
                fn to_bytes(&self, endianness: Endianness) -> Vec<u8> {
                    match endianness {
                        Endianness::Little => self.to_le_bytes().to_vec(),
                        Endianness::Big => self.to_be_bytes().to_vec(),
                        Endianness::Native => self.to_ne_bytes().to_vec(),
                    }
                }
            }
        )*
    };
}

impl_to_bytes_for_primitive!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize, f32, f64);

#[derive(Debug, Clone, Copy)]
enum IntType {
    U8,
    U16,
    U32,
    U64,
    USize,
    I8,
    I16,
    I32,
    I64,
    ISize,
}

impl IntType {
    fn from_suffix(suffix: &str) -> Option<Self> {
        match suffix {
            "u8" => Some(IntType::U8),
            "u16" => Some(IntType::U16),
            "u32" => Some(IntType::U32),
            "u64" => Some(IntType::U64),
            "usize" => Some(IntType::USize),
            "i8" => Some(IntType::I8),
            "i16" => Some(IntType::I16),
            "i32" => Some(IntType::I32),
            "i64" => Some(IntType::I64),
            "isize" => Some(IntType::ISize),
            _ => None,
        }
    }
}

fn base10_digits_to_bytes(
    digits: &str,
    int_type: IntType,
    endianness: Endianness,
) -> syn::Result<Vec<u8>> {
    macro_rules! parse_int {
        ($t:ty, $digits:expr) => {{
            let value: $t = $digits.parse().map_err(|e| {
                syn::Error::new(
                    proc_macro2::Span::call_site(),
                    format!("Failed to parse {}: {e}", stringify!($t)),
                )
            })?;
            Ok(ToBytes::to_bytes(&value, endianness))
        }};
    }
    match int_type {
        IntType::U8 => parse_int!(u8, digits),
        IntType::U16 => parse_int!(u16, digits),
        IntType::U32 => parse_int!(u32, digits),
        IntType::U64 => parse_int!(u64, digits),
        IntType::USize => parse_int!(usize, digits),
        IntType::I8 => parse_int!(i8, digits),
        IntType::I16 => parse_int!(i16, digits),
        IntType::I32 => parse_int!(i32, digits),
        IntType::I64 => parse_int!(i64, digits),
        IntType::ISize => parse_int!(isize, digits),
    }
}

#[proc_macro]
pub fn datalit(input: BaseTokenStream) -> BaseTokenStream {
    datalit_impl(input.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

fn parse_int_literal(lit: LitInt) -> syn::Result<Vec<u8>> {
    let mut suffix = lit.suffix();

    if suffix.is_empty() {
        // Check to see if the representation is a hexidecimal literal
        let literal_digits = lit.to_string().to_ascii_lowercase();
        if literal_digits.starts_with("0x") {
            let hex_digits = literal_digits.trim_start_matches("0x").replace('_', "");
            // This should be a valid hex string, which should be in ascii.
            // We can use the byte length to determine how many chars were used.
            // We need an even number of hex digits to form bytes.
            if hex_digits.len() % 2 != 0 {
                return Err(syn::Error::new_spanned(
                    lit,
                    format!("Hex literal must have an even number of digits to form bytes. Has {} digits.", hex_digits.len()),
                ));
            }
            return (0..hex_digits.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex_digits[i..i + 2], 16))
                .collect::<Result<Vec<u8>, _>>()
                .map_err(|e| syn::Error::new_spanned(lit, format!("Invalid hex literal: {e}")));
        }
    }

    let endianness = if suffix.ends_with("le") {
        suffix = suffix.trim_end_matches("le");
        suffix = suffix.trim_end_matches('_');
        Endianness::Little
    } else if suffix.ends_with("be") {
        suffix = suffix.trim_end_matches("be");
        suffix = suffix.trim_end_matches('_');
        Endianness::Big
    } else {
        Endianness::Native
    };

    let int_type = IntType::from_suffix(suffix).ok_or_else(|| {
        syn::Error::new_spanned(
            &lit,
            format!("Invalid or missing integer type suffix: '{}'", lit.suffix()),
        )
    })?;

    base10_digits_to_bytes(lit.base10_digits(), int_type, endianness)
}

#[derive(derive_syn_parse::Parse)]
enum DataLitEntry {
    #[peek(LitInt, name = "integer literal")]
    Int(LitInt),
    #[peek(LitByteStr, name = "byte string literal")]
    ByteStr(LitByteStr),
    #[peek(LitByte, name = "byte literal")]
    Byte(LitByte),
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
        let stmt = match entry {
            DataLitEntry::Int(lit) => {
                let bytes: Vec<_> = parse_int_literal(lit)?
                    .into_iter()
                    .map(Literal::u8_suffixed)
                    .collect();
                quote! { #data_var.extend_from_slice(&[#(#bytes),*]); }
            }
            DataLitEntry::ByteStr(lit) => {
                let bytes = lit.value();
                quote! { #data_var.extend_from_slice(&[#(#bytes),*]); }
            }
            DataLitEntry::Byte(lit) => {
                let byte = lit.value();
                quote! { #data_var.push(#byte); }
            }
        };
        data_statements.push(stmt);
    }
    Ok(quote! {
        {
            let mut #data_var: Vec<u8> = Vec::new();
            #(#data_statements)*
            #data_var
        }
    })
}
