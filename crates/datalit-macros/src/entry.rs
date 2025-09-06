use proc_macro2::{Literal, TokenStream};
use quote::{ToTokens, quote};
use syn::{Error, Ident, LitByte, LitByteStr, LitCStr, LitInt, Result};

use crate::{Endianness, IntType, base10_digits_to_bytes};

fn parse_byte_literal<T>(
    err_context: &T,
    digit_type_name: &str,
    digits: &str,
    digits_per_byte: usize,
) -> Result<Vec<u8>>
where
    T: ToTokens,
{
    // This should be a valid hex string, which should be in ascii.
    // We can use the byte length to determine how many chars were used.
    // We need an even number of hex digits to form bytes.
    assert_eq!(8 % digits_per_byte, 0);
    if digits.len() % digits_per_byte != 0 {
        return Err(Error::new_spanned(
            err_context,
            format!(
                "{} literal must have an even number of digits to form bytes. Has {} digits.",
                digit_type_name,
                digits.len()
            ),
        ));
    }
    (0..digits.len())
        .step_by(digits_per_byte)
        .map(|i| {
            u8::from_str_radix(
                &digits[i..i + digits_per_byte],
                2u32.pow((8 / digits_per_byte) as u32),
            )
        })
        .collect::<std::result::Result<Vec<u8>, _>>()
        .map_err(|e| {
            Error::new_spanned(
                err_context,
                format!("Invalid {} literal: {e}", digit_type_name.to_lowercase()),
            )
        })
}

fn parse_int_literal(lit: LitInt) -> Result<Vec<u8>> {
    let mut suffix = lit.suffix();

    if suffix.is_empty() {
        // Check to see if the representation is a hexidecimal literal
        let literal_digits = lit.to_string().to_ascii_lowercase();
        if literal_digits.starts_with("0x") {
            // This should be a valid hex string, which should be in ascii.
            // We can use the byte length to determine how many chars were used.
            // We need an even number of hex digits to form bytes.
            let hex_digits = literal_digits.trim_start_matches("0x").replace('_', "");
            return parse_byte_literal(&lit, "Hex", &hex_digits, 2);
        } else if literal_digits.starts_with("0b") {
            let bin_digits = literal_digits.trim_start_matches("0b").replace('_', "");
            return parse_byte_literal(&lit, "Binary", &bin_digits, 8);
        } else {
            return Err(Error::new_spanned(
                &lit,
                "Integer literal must have a type suffix (e.g. 'u8', 'i32', etc.) or be a hex (0x...) or binary (0b...) literal",
            ));
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
        Error::new_spanned(
            &lit,
            format!("Invalid or missing integer type suffix: '{}'", lit.suffix()),
        )
    })?;

    base10_digits_to_bytes(lit.base10_digits(), int_type, endianness)
}

fn new_literal_bytes_stmt(data_var: &Ident, bytes: &[u8]) -> TokenStream {
    let byte_literals: Vec<_> = bytes.iter().map(|b| Literal::u8_suffixed(*b)).collect();
    quote! {
        #data_var.extend_from_slice(&[#(#byte_literals),*]);
    }
}

#[derive(derive_syn_parse::Parse)]
pub enum DataLitEntry {
    #[peek(LitInt, name = "integer literal")]
    Int(LitInt),
    #[peek(LitByteStr, name = "byte string literal")]
    ByteStr(LitByteStr),
    #[peek(LitByte, name = "byte literal")]
    Byte(LitByte),
    #[peek(LitCStr, name = "C-style string literal")]
    CStr(LitCStr),
}

impl DataLitEntry {
    pub fn into_tokens(self, data_var: &Ident) -> Result<TokenStream> {
        Ok(match self {
            DataLitEntry::Int(lit) => {
                let bytes: Vec<_> = parse_int_literal(lit)?;
                new_literal_bytes_stmt(data_var, &bytes)
            }
            DataLitEntry::ByteStr(lit) => {
                let bytes = lit.value();
                new_literal_bytes_stmt(data_var, &bytes)
            }
            DataLitEntry::Byte(lit) => {
                let byte = lit.value();
                quote! { #data_var.push(#byte); }
            }
            DataLitEntry::CStr(lit) => {
                let c_string = lit.value();
                let bytes = c_string.as_bytes_with_nul();
                new_literal_bytes_stmt(data_var, bytes)
            }
        })
    }
}
