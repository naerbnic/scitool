use std::ops::{Bound, RangeBounds};

use bytes::BufMut;

pub trait BufferOpsExt {
    fn read_u16_le_at(&self, offset: usize) -> u16;
}

impl BufferOpsExt for [u8] {
    fn read_u16_le_at(&self, offset: usize) -> u16 {
        let bytes = &self[offset..][..2];
        u16::from_le_bytes(bytes.try_into().unwrap())
    }
}

pub trait ToFixedBytes {
    const SIZE: usize;
    fn to_bytes(&self, dest: &mut [u8]) -> anyhow::Result<()>;
}

pub trait FromFixedBytes: Sized {
    const SIZE: usize;
    fn parse<B: bytes::Buf>(bytes: B) -> anyhow::Result<Self>;
}

macro_rules! impl_fixed_bytes_for_num {
    ($($num:ty),*) => {
        $(
            impl ToFixedBytes for $num {
                const SIZE: usize = std::mem::size_of::<$num>();

                fn to_bytes(&self, dest: &mut [u8]) -> anyhow::Result<()> {
                    dest.copy_from_slice(&self.to_le_bytes());
                    Ok(())
                }
            }

            impl FromFixedBytes for $num {
                const SIZE: usize = std::mem::size_of::<$num>();

                fn parse<B: bytes::Buf>(bytes: B) -> anyhow::Result<Self> {
                    let mut byte_array = [0u8; <Self as FromFixedBytes>::SIZE];
                    (&mut byte_array[..]).put(bytes);
                    Ok(Self::from_le_bytes(byte_array))
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
    type Error: std::error::Error + Send + Sync + 'static;
    type Guard<'g>: bytes::Buf
    where
        Self: 'g;

    fn size(&self) -> u64;
    #[must_use]
    fn sub_buffer_from_range(self, start: u64, end: u64) -> Self;
    fn split_at(self, at: u64) -> (Self, Self);
    fn lock_range(&self, start: u64, end: u64) -> Result<Self::Guard<'_>, Self::Error>;

    fn lock(&self) -> Result<Self::Guard<'_>, Self::Error> {
        self.lock_range(0, self.size())
    }

    /// Reads a value from the front of the buffer, returning the value and the
    /// remaining buffer.
    // Functions that can be implemented in terms of the above functions.
    fn read_value<T: FromFixedBytes>(self) -> anyhow::Result<(T, Self)> {
        let (first, second) = self.split_at(T::SIZE.try_into().unwrap());
        T::parse(first.lock()?).map(|value| (value, second))
    }

    fn is_empty(&self) -> bool {
        self.size() == 0
    }

    /// Splits the block into chunks of the given size. Panics if the block size
    /// is not a multiple of the chunk size.
    fn split_chunks(self, chunk_size: u64) -> Vec<Self> {
        let mut remaining = self;
        let mut chunks = Vec::new();
        while remaining.size() != 0 {
            assert!(remaining.size() >= chunk_size);
            let (chunk, new_remaining) = remaining.split_at(chunk_size);
            chunks.push(chunk);
            remaining = new_remaining;
        }
        chunks
    }

    fn split_values<T: FromFixedBytes>(self) -> anyhow::Result<Vec<T>> {
        let buf_size = self.size();
        let item_size: u64 = T::SIZE.try_into().unwrap();
        assert!((buf_size % item_size) == 0);
        let (values, rest) = self.read_values::<T>((buf_size / item_size).try_into().unwrap())?;
        assert!(rest.is_empty());
        Ok(values)
    }

    /// Reads N values from the front of the buffer, returning the values and the
    /// remaining buffer.
    fn read_values<T: FromFixedBytes>(self, count: usize) -> anyhow::Result<(Vec<T>, Self)> {
        let mut values = Vec::with_capacity(count);
        let mut remaining = self;
        for _ in 0..count {
            let (value, new_remaining) = remaining.read_value()?;
            values.push(value);
            remaining = new_remaining;
        }
        Ok((values, remaining))
    }

    fn read_length_delimited_block(self, item_size: u64) -> anyhow::Result<(Self, Self)> {
        let (num_blocks, next) = self.read_value::<u16>()?;
        let total_block_size = u64::from(num_blocks).checked_mul(item_size).unwrap();
        Ok(next.split_at(total_block_size))
    }

    /// Reads a sequence of values, where the first value is a little endian
    /// u16 indicating the number of values to read.
    fn read_length_delimited_records<T: FromFixedBytes>(self) -> anyhow::Result<(Vec<T>, Self)> {
        let (num_records, next) = self.read_value::<u16>()?;
        let (values, next) = next.read_values::<T>(num_records as usize)?;
        Ok((values, next))
    }

    fn to_vec(&self) -> Result<Vec<u8>, Self::Error> {
        let mut vec = Vec::with_capacity(self.size().try_into().unwrap());
        vec.put(self.lock()?);
        Ok(vec)
    }
}

impl Buffer for &[u8] {
    type Error = NoError;
    type Guard<'g>
        = &'g [u8]
    where
        Self: 'g;
    fn size(&self) -> u64 {
        self.len().try_into().unwrap()
    }

    fn sub_buffer_from_range(self, start: u64, end: u64) -> Self {
        let start = start.try_into().unwrap();
        let end = end.try_into().unwrap();
        &self[start..end]
    }

    fn split_at(self, at: u64) -> (Self, Self) {
        (*self).split_at(at.try_into().unwrap())
    }

    fn lock_range(&self, start: u64, end: u64) -> Result<Self::Guard<'_>, NoError> {
        let start = usize::try_from(start).unwrap();
        let end = usize::try_from(end).unwrap();
        Ok(&self[start..end])
    }

    fn read_value<T: FromFixedBytes>(self) -> anyhow::Result<(T, Self)> {
        let (first, second) = self.split_at(T::SIZE);
        T::parse(first).map(|value| (value, second))
    }

    fn read_values<T: FromFixedBytes>(self, count: usize) -> anyhow::Result<(Vec<T>, Self)> {
        let mut values = Vec::with_capacity(count);
        let mut remaining = self;
        for _ in 0..count {
            let (value, new_remaining) = remaining.read_value()?;
            values.push(value);
            remaining = new_remaining;
        }
        Ok((values, remaining))
    }
}

pub trait BufferExt: Buffer {
    #[must_use]
    fn sub_buffer<T, R: RangeBounds<T>>(self, range: R) -> Self
    where
        T: TryInto<u64> + num::Num + Copy,
        T::Error: std::fmt::Debug,
    {
        let start = match range.start_bound() {
            Bound::Included(&start) => start.try_into().unwrap(),
            Bound::Excluded(&start) => (start + T::one()).try_into().unwrap(),
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(&end) => (end + T::one()).try_into().unwrap(),
            Bound::Excluded(&end) => end.try_into().unwrap(),
            Bound::Unbounded => self.size(),
        };
        self.sub_buffer_from_range(start, end)
    }
}

impl<T: Buffer> BufferExt for T {}
