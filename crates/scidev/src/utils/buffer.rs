use std::ops::{Bound, RangeBounds};

use bytes::BufMut;

pub trait ToFixedBytes {
    const SIZE: usize;
    fn to_bytes(&self, dest: &mut [u8]);
}

pub trait FromFixedBytes: Sized {
    const SIZE: usize;
    fn parse<B: bytes::Buf>(bytes: B) -> Self;
}

macro_rules! impl_fixed_bytes_for_num {
    ($($num:ty),*) => {
        $(
            impl ToFixedBytes for $num {
                const SIZE: usize = std::mem::size_of::<$num>();

                fn to_bytes(&self, dest: &mut [u8]) {
                    dest.copy_from_slice(&self.to_le_bytes());
                }
            }

            impl FromFixedBytes for $num {
                const SIZE: usize = std::mem::size_of::<$num>();

                fn parse<B: bytes::Buf>(bytes: B) -> Self {
                    let mut byte_array = [0u8; <Self as FromFixedBytes>::SIZE];
                    (&mut byte_array[..]).put(bytes);
                    Self::from_le_bytes(byte_array)
                }
            }
        )*
    };
}

impl_fixed_bytes_for_num!(i8, i16, i32, i64, i128, isize);
impl_fixed_bytes_for_num!(u8, u16, u32, u64, u128, usize);

pub trait Index: num::Num + std::fmt::Debug + Ord + Copy {
    const BITS: u32;
    fn widen_to_usize(self) -> usize;
    fn narrow_from_usize(idx: usize) -> Option<Self>;
    fn widen_to<T: Index>(self) -> T {
        assert!(Self::BITS <= T::BITS);
        T::narrow_from_usize(self.widen_to_usize()).unwrap()
    }
    fn narrow_to<T: Index>(self) -> Option<T> {
        assert!(T::BITS <= Self::BITS);
        T::narrow_from_usize(self.widen_to_usize())
    }
}

macro_rules! impl_index {
    ($($ty:ty),*) => {
        $(
            #[allow(
                clippy::cast_possible_truncation,
                clippy::checked_conversions,
            )]
            impl Index for $ty {
                const BITS: u32 = std::mem::size_of::<$ty>() as u32 * 8;

                fn widen_to_usize(self) -> usize {
                    assert!(<$ty>::BITS <= usize::BITS);
                    self as usize
                }

                fn narrow_from_usize(idx: usize) -> Option<Self> {
                    assert!(<$ty>::BITS <= usize::BITS);
                    if idx <= <$ty>::MAX as usize {
                        Some(idx as $ty)
                    } else {
                        None
                    }
                }
            }
        )*
    };
}

impl_index!(u8, u16, u32, u64, u128, usize);

pub trait NarrowedIndex: Index + Sized + Copy {
    fn widened_max_size() -> usize;
}

macro_rules! impl_narrowed_index {
    ($($small:ty),*) => {
        $(
            #[allow(clippy::cast_possible_truncation)]
            impl NarrowedIndex for $small {
                fn widened_max_size() -> usize {
                    assert!(<$small>::BITS <= usize::BITS);
                    let max = <$small>::MAX as usize;
                        if max == usize::MAX {
                            max
                        } else {
                            max + 1
                        }
                    }
                }
        )*
    };
}

impl_narrowed_index!(u16, u32, usize);

#[cfg(target_pointer_width = "64")]
impl_narrowed_index!(u64);

#[derive(Debug, Copy, Clone)]
pub enum NoError {}

impl std::fmt::Display for NoError {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {}
    }
}
impl std::error::Error for NoError {}

pub trait NoErrorResultExt<T> {
    fn into_ok(self) -> T;
}

impl<T> NoErrorResultExt<T> for Result<T, NoError> {
    fn into_ok(self) -> T {
        match self {
            Ok(value) => value,
        }
    }
}

/// An abstraction over types that contain a buffer of bytes.
///
/// This is designed to be usable with both mutable and immutable byte
/// buffers, and both owned and borrowed buffers.
///
/// Each buffer specifies its own index type, used as a byte offset
/// into the buffer.
pub trait Buffer: Sized + Clone {
    #[must_use]
    fn sub_buffer_from_range(&self, start: usize, end: usize) -> Self;
    fn get_slice(&self, offset: usize, len: usize) -> &[u8];
    fn size(&self) -> usize;
}

impl Buffer for &[u8] {
    fn sub_buffer_from_range(&self, start: usize, end: usize) -> Self {
        assert!(start <= end);
        assert!(end <= self.len());
        &self[start..end]
    }

    fn get_slice(&self, offset: usize, len: usize) -> &[u8] {
        assert!(offset + len <= self.len());
        &self[offset..offset + len]
    }

    fn size(&self) -> usize {
        self.len()
    }
}

pub trait BufferExt: Buffer {
    #[must_use]
    fn sub_buffer<T, R: RangeBounds<T>>(self, range: R) -> Self
    where
        T: Into<usize> + Copy,
    {
        let start = match range.start_bound() {
            Bound::Included(&start) => start.into(),
            Bound::Excluded(&start) => start.into() + 1,
            Bound::Unbounded => 0usize,
        };

        let end = match range.end_bound() {
            Bound::Included(&end) => end.into() + 1,
            Bound::Excluded(&end) => end.into(),
            Bound::Unbounded => self.size(),
        };

        assert!(start <= end);

        assert!(start <= end);
        self.sub_buffer_from_range(start, end)
    }

    fn as_slice(&self) -> &[u8] {
        self.get_slice(0, self.size())
    }
}

impl<T: Buffer> BufferExt for T {}
