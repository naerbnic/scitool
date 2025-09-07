

#[derive(Debug, Clone, Copy)]
pub enum Endianness {
    Little,
    Big,
    Native,
}

pub trait ToBytes {
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
pub enum IntType {
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
    pub fn from_suffix(suffix: &str) -> Option<Self> {
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

    pub fn to_type(self) -> syn::Ident {
        let ident = match self {
            IntType::U8 => "u8",
            IntType::U16 => "u16",
            IntType::U32 => "u32",
            IntType::U64 => "u64",
            IntType::USize => "usize",
            IntType::I8 => "i8",
            IntType::I16 => "i16",
            IntType::I32 => "i32",
            IntType::I64 => "i64",
            IntType::ISize => "isize",
        };
        quote::format_ident!("{}", ident)
    }

    pub fn to_byte_size(self) -> usize {
        match self {
            IntType::U8 | IntType::I8 => 1,
            IntType::U16 | IntType::I16 => 2,
            IntType::U32 | IntType::I32 => 4,
            IntType::U64 | IntType::I64 => 8,
            IntType::USize | IntType::ISize => std::mem::size_of::<usize>(),
        }
    }
}

pub fn base10_digits_to_bytes(
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