#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signedness {
    Signed,
    Unsigned,
}

impl Signedness {
    #[allow(
        dead_code,
        reason = "This is used in a static assert within a macro below"
    )]
    const fn is_signed(self) -> bool {
        match self {
            Signedness::Signed => true,
            Signedness::Unsigned => false,
        }
    }
}

/// A type that all numbers can be widened to, without losing any information
/// about the source number.
///
/// We assume that the compiler will optimize the code to remove the overhead of
/// this struct.
pub struct WidestInt {
    /// The signedness of the source number. This is used to ensure that
    /// the meaning of the number is preserved when converting back to the
    /// original type.
    signedness: Signedness,

    /// The content of the integer. If the original number was signed, this
    /// is in 2's complement form, otherwise the data is unsigned.
    value: u64,
}

pub trait NumConvert: num::Num {
    const SIGNEDNESS: Signedness;
    const BITS: u32;
    fn convert_to_wide(self) -> WidestInt;
    fn safe_convert_from_widest(widest: WidestInt) -> anyhow::Result<Self>;

    fn convert_num_to<T: NumConvert>(self) -> anyhow::Result<T> {
        let wide = self.convert_to_wide();
        T::safe_convert_from_widest(wide)
    }
}

macro_rules! impl_bit_convert_unsigned {
    ($($t:ty),*) => {
        $(
            #[allow(clippy::cast_possible_truncation, clippy::cast_lossless, clippy::checked_conversions)]
            impl NumConvert for $t {
                const SIGNEDNESS: Signedness = Signedness::Unsigned;
                const BITS: u32 = <$t>::BITS;
                fn convert_to_wide(self) -> WidestInt {
                    WidestInt {
                        signedness: Signedness::Unsigned,
                        value: self as u64,
                    }
                }

                fn safe_convert_from_widest(widest: WidestInt) -> anyhow::Result<Self> {
                    if let Signedness::Signed = widest.signedness {
                        // This can be valid as long as the sign bit is not set.
                        anyhow::ensure!(
                            widest.value & (1 << (u64::BITS - 1)) == 0,
                            "number {} is negative, which cannot fit in an unsigned number",
                            widest.value
                        );
                    }
                    anyhow::ensure!(
                        widest.value <= <$t>::MAX as u64,
                        "number {} is too large to fit in {}",
                        widest.value,
                        std::any::type_name::<$t>()
                    );
                    Ok(widest.value as $t)
                }
            }
        )*
    };
}

macro_rules! impl_bit_convert_signed {
    ($($t:ty),*) => {
        $(
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_lossless,
                clippy::cast_sign_loss,
                clippy::checked_conversions,
                clippy::cast_possible_wrap
            )]
            impl NumConvert for $t {
                const SIGNEDNESS: Signedness = Signedness::Signed;
                const BITS: u32 = <$t>::BITS;
                fn convert_to_wide(self) -> WidestInt {
                    WidestInt {
                        signedness: Signedness::Signed,
                        value: self as i64 as u64,
                    }
                }

                fn safe_convert_from_widest(widest: WidestInt) -> anyhow::Result<Self> {
                    match widest.signedness {
                        Signedness::Signed => {
                            let signed_value = widest.value as i64;
                            anyhow::ensure!(
                                signed_value <= <$t>::MAX as i64,
                                "number {} is too large to fit in {}",
                                widest.value,
                                std::any::type_name::<$t>()
                            );
                            Ok(signed_value as $t)
                        }
                        Signedness::Unsigned => {
                            anyhow::ensure!(
                                widest.value <= <$t>::MAX as u64,
                                "number {} is too large to fit in {}",
                                widest.value,
                                std::any::type_name::<$t>()
                            );
                            Ok(widest.value as $t)
                        }
                    }
                }
            }
        )*
    };
}

impl_bit_convert_unsigned!(u8, u16, u32, u64, usize);
impl_bit_convert_signed!(i8, i16, i32, i64, isize);

pub trait WidenTo<T>: Copy {
    fn safe_widen_to(self) -> T;
}

pub trait WidenFrom<T>: Sized + Copy {
    fn safe_widen_from(value: T) -> Self;
}

impl<T: WidenFrom<U>, U> WidenTo<T> for U
where
    U: Copy,
{
    fn safe_widen_to(self) -> T {
        T::safe_widen_from(self)
    }
}

macro_rules! impl_widen_to {
    ($($t:ty => ($($to:ty),*)),*) => {
        $(
            $(
                const _: () = assert!((<$to>::BITS > <$t>::BITS) || (<$t>::BITS == <$to>::BITS && <$t>::SIGNEDNESS.is_signed() == <$to>::SIGNEDNESS.is_signed()),
                    "invalid widen impl");
                impl WidenFrom<$t> for $to {
                    fn safe_widen_from(value: $t) -> Self {
                        <$to>::safe_convert_from_widest(NumConvert::convert_to_wide(value)).unwrap()
                    }
                }
            )*
        )*
    };
}

impl_widen_to!(u8 => (u8, u16, i16, u32, i32, u64, i64, usize, isize),
               u16 => (u16, u32, i32, u64, i64, usize, isize),
               u32 => (u32, u64, i64, usize, isize),
               u64 => (u64),
               i8 => (i16, i32, i64, isize),
               i16 => (i32, i64, isize),
               i32 => (i64, isize),
               i64 => (i64));

#[cfg(test)]
mod tests {
    use super::{NumConvert, WidenFrom};
    #[test]
    fn self_widen_works() {
        assert_eq!(u8::safe_widen_from(1u8), 1u8);
        assert_eq!(u16::safe_widen_from(1u8), 1u16);
        assert_eq!(u32::safe_widen_from(1u8), 1u32);
        assert_eq!(u64::safe_widen_from(1u8), 1u64);
        assert_eq!(usize::safe_widen_from(1u8), 1usize);
    }

    #[test]
    fn convert_positive_signed_to_unsigned_works() {
        assert!(matches!(127i8.convert_num_to::<u8>(), Ok(127u8)));
    }

    #[test]
    fn convert_large_positive_unsigned_to_signed_fails() {
        assert!(255u8.convert_num_to::<i8>().is_err());
    }
    #[test]
    fn convert_large_positive_unsigned_to_wider_signed_works() {
        assert!(matches!(255u8.convert_num_to::<i16>(), Ok(255i16)));
    }

    #[test]
    fn convert_negative_signed_to_unsigned_fails() {
        assert!((-1i8).convert_num_to::<u8>().is_err());
    }
}
