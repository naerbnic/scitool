use std::ops::{Bound, RangeBounds};

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
    fn parse(bytes: &[u8]) -> anyhow::Result<Self>;
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

                fn parse(bytes: &[u8]) -> anyhow::Result<Self> {
                    let bytes = bytes.try_into().unwrap();
                    Ok(Self::from_le_bytes(bytes))
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

/// An abstraction over types that contain a buffer of bytes.
///
/// This is designed to be usable with both mutable and immutable byte
/// buffers, and both owned and borrowed buffers.
///
/// Each buffer specifies its own index type, used as a byte offset
/// into the buffer.
pub trait Buffer<'a>: Sized + AsRef<[u8]> {
    type Idx: Index;

    fn size(&self) -> usize;
    fn sub_buffer<R: RangeBounds<Self::Idx>>(self, range: R) -> Self;
    fn split_at(self, at: Self::Idx) -> (Self, Self);

    /// Reads a value from the front of the buffer, returning the value and the
    /// remaining buffer.
    fn read_value<T: FromFixedBytes>(self) -> anyhow::Result<(T, Self)>;

    // Functions that can be implemented in terms of the above functions.

    fn is_empty(&self) -> bool {
        self.size() == 0
    }

    /// Splits the block into chunks of the given size. Panics if the block size
    /// is not a multiple of the chunk size.
    fn split_chunks(self, chunk_size: usize) -> Vec<Self> {
        let mut remaining = self;
        let mut chunks = Vec::new();
        while remaining.size() != 0usize {
            assert!(remaining.size() >= chunk_size);
            let (chunk, new_remaining) =
                remaining.split_at(Self::Idx::narrow_from_usize(chunk_size).unwrap());
            chunks.push(chunk);
            remaining = new_remaining;
        }
        chunks
    }

    fn split_values<T: FromFixedBytes>(self) -> anyhow::Result<Vec<T>> {
        let buf_size = self.size();
        assert!(buf_size % T::SIZE == 0);
        let (values, rest) = self.read_values::<T>(buf_size / T::SIZE)?;
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

    /// Reads a sequence of values, where the first value is a little endian
    /// u16 indicating the number of values to read.
    fn read_length_delimited_records<T: FromFixedBytes>(self) -> anyhow::Result<(Vec<T>, Self)> {
        let (num_records, next) = self.read_value::<u16>()?;
        let (values, next) = next.read_values::<T>(num_records as usize)?;
        Ok((values, next))
    }

    // Functions to create other dervied buffers.

    fn narrow<Idx: NarrowedIndex>(self) -> NarrowedIndexBuffer<'a, Idx, Self> {
        NarrowedIndexBuffer::new(self)
    }
}

impl<'a> Buffer<'a> for &'a [u8] {
    type Idx = usize;
    fn size(&self) -> usize {
        self.len()
    }

    fn sub_buffer<R: RangeBounds<usize>>(self, range: R) -> Self {
        let start = match range.start_bound() {
            Bound::Included(&start) => start,
            Bound::Excluded(&start) => start + 1,
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(&end) => end + 1,
            Bound::Excluded(&end) => end,
            Bound::Unbounded => self.len(),
        };
        &self[start..end]
    }

    fn split_at(self, at: usize) -> (Self, Self) {
        (*self).split_at(at)
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

impl<'a> Buffer<'a> for &'a mut [u8] {
    type Idx = usize;
    fn size(&self) -> usize {
        self.len()
    }
    fn sub_buffer<R: RangeBounds<usize>>(self, range: R) -> Self {
        let start = match range.start_bound() {
            Bound::Included(&start) => start,
            Bound::Excluded(&start) => start + 1,
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(&end) => end + 1,
            Bound::Excluded(&end) => end,
            Bound::Unbounded => self.len(),
        };
        &mut self[start..end]
    }

    fn split_at(self, at: usize) -> (Self, Self) {
        self.split_at_mut(at)
    }

    fn read_value<T: FromFixedBytes>(self) -> anyhow::Result<(T, Self)> {
        let (first, second) = self.split_at_mut(T::SIZE);
        T::parse(first).map(|value| (value, second))
    }
}

pub struct NarrowedIndexBuffer<'a, Idx, B> {
    buffer: B,
    _phantom: std::marker::PhantomData<(Idx, &'a u8)>,
}

impl<'a, Idx, B> Clone for NarrowedIndexBuffer<'a, Idx, B>
where
    B: Clone,
{
    fn clone(&self) -> Self {
        NarrowedIndexBuffer {
            buffer: self.buffer.clone(),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<'a, Idx, B> NarrowedIndexBuffer<'a, Idx, B>
where
    B: Buffer<'a>,
    Idx: NarrowedIndex,
{
    pub fn new(buffer: B) -> Self {
        assert!(buffer.size() <= Idx::widened_max_size());
        assert!(B::Idx::BITS >= Idx::BITS);
        NarrowedIndexBuffer {
            buffer,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<'a, Idx, B> std::ops::Deref for NarrowedIndexBuffer<'a, Idx, B> {
    type Target = B;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl<'a, Idx, B> AsRef<[u8]> for NarrowedIndexBuffer<'a, Idx, B>
where
    B: Buffer<'a>,
{
    fn as_ref(&self) -> &[u8] {
        self.buffer.as_ref()
    }
}

impl<'a, Idx, B> Buffer<'a> for NarrowedIndexBuffer<'a, Idx, B>
where
    B: Buffer<'a>,
    Idx: NarrowedIndex + num::Zero,
{
    type Idx = Idx;

    fn size(&self) -> usize {
        self.buffer.size()
    }

    fn sub_buffer<R: RangeBounds<Self::Idx>>(self, range: R) -> Self {
        let start: Bound<B::Idx> = match range.start_bound() {
            Bound::Included(&start) => Bound::Included(start.widen_to()),
            Bound::Excluded(&start) => Bound::Excluded(start.widen_to()),
            Bound::Unbounded => Bound::Unbounded,
        };

        let end: Bound<B::Idx> = match range.end_bound() {
            Bound::Included(&end) => Bound::Included(end.widen_to()),
            Bound::Excluded(&end) => Bound::Excluded(end.widen_to()),
            Bound::Unbounded => Bound::Unbounded,
        };
        NarrowedIndexBuffer {
            buffer: self.buffer.sub_buffer((start, end)),
            _phantom: std::marker::PhantomData,
        }
    }

    fn split_at(self, at: Idx) -> (Self, Self) {
        let (first, second) = self.buffer.split_at(at.widen_to());
        (
            NarrowedIndexBuffer {
                buffer: first,
                _phantom: std::marker::PhantomData,
            },
            NarrowedIndexBuffer {
                buffer: second,
                _phantom: std::marker::PhantomData,
            },
        )
    }

    fn read_value<T: FromFixedBytes>(self) -> anyhow::Result<(T, Self)> {
        self.buffer.read_value().map(|(value, remaining)| {
            (
                value,
                NarrowedIndexBuffer {
                    buffer: remaining,
                    _phantom: std::marker::PhantomData,
                },
            )
        })
    }
}
