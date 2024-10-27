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

pub trait FromFixedBytes: ToFixedBytes + Sized {
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

/// A type that allows for the full range of buffer sizes for a given index
/// type, including the maximum size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BufferSize<T> {
    /// The buffer is a value in [0, MAX_SIZE).
    Size(T),
    /// The buffer is exactly MAX_SIZE bytes.
    Max,
}

impl<T> From<T> for BufferSize<T> {
    fn from(size: T) -> Self {
        BufferSize::Size(size)
    }
}

pub trait NarrowedIndex<LargerIdx>: Sized + Copy {
    fn widened_max_size() -> BufferSize<LargerIdx>;
    fn widen_to(self) -> LargerIdx;
    fn narrow_from(idx: LargerIdx) -> Option<Self>;
    fn narrow_size_from(idx: BufferSize<LargerIdx>) -> Option<BufferSize<Self>>;
}

macro_rules! impl_narrowed_index {
    ($($small:ty => ($($large:ty),*)),*) => {
        $(
            $(
                impl NarrowedIndex<$large> for $small {
                    fn widened_max_size() -> BufferSize<$large> {
                        assert!(<$small>::BITS <= <$large>::BITS);
                        if (<$small>::BITS == <$large>::BITS) {
                            return BufferSize::Max;
                        } else {
                            BufferSize::Size(1 as $large << <$small>::BITS)
                        }
                    }

                    fn widen_to(self) -> $large {
                        assert!(<$small>::BITS <= <$large>::BITS);
                        self as $large
                    }

                    fn narrow_from(idx: $large) -> Option<Self> {
                        assert!(<$small>::BITS <= <$large>::BITS);
                        if idx <= <$small>::MAX as $large {
                            Some(idx as $small)
                        } else {
                            None
                        }
                    }

                    fn narrow_size_from(idx: BufferSize<$large>) -> Option<BufferSize<Self>> {
                        assert!(<$small>::BITS <= <$large>::BITS);
                        match idx {
                            BufferSize::Size(idx) => {
                                if idx <= <$small>::MAX as $large {
                                    Some(BufferSize::Size(idx as $small))
                                } else {
                                    None
                                }
                            }
                            BufferSize::Max => {
                                if <$small>::BITS == <$large>::BITS {
                                    Some(BufferSize::Max)
                                } else {
                                    None
                                }
                            }
                        }
                    }
                }
            )*
        )*
    };
}

impl_narrowed_index!(
    u16 => (u16, u32, u64, u128, usize),
    u32 => (u32, u64, u128, usize),
    u64 => (u64, u128),
    usize => (u64, usize)
);

#[cfg(target_pointer_width = "32")]
impl_narrowed_index!(usize => (u32));

#[cfg(target_pointer_width = "64")]
impl_narrowed_index!(u64 => (usize));

/// An abstraction over types that contain a buffer of bytes.
///
/// This is designed to be usable with both mutable and immutable byte
/// buffers, and both owned and borrowed buffers.
///
/// Each buffer specifies its own index type, used as a byte offset
/// into the buffer.
pub trait Buffer<'a>: Sized + AsRef<[u8]> {
    type Idx;

    fn size(&self) -> BufferSize<Self::Idx>;
    fn sub_buffer<R: RangeBounds<Self::Idx>>(self, range: R) -> Self;
    fn buf_split_at(self, at: impl Into<BufferSize<Self::Idx>>) -> (Self, Self);
    /// Reads a value from the front of the buffer, returning the value and the
    /// remaining buffer.
    fn read_value<T: FromFixedBytes>(self) -> anyhow::Result<(T, Self)>;
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

    // Functions to create other dervied buffers.
    fn narrow<Idx: NarrowedIndex<Self::Idx>>(self) -> NarrowedIndexBuffer<'a, Idx, Self> {
        NarrowedIndexBuffer::new(self)
    }
}

impl<'a> Buffer<'a> for &'a [u8] {
    type Idx = usize;
    fn size(&self) -> BufferSize<Self::Idx> {
        BufferSize::Size(self.len())
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

    fn buf_split_at(self, at: impl Into<BufferSize<usize>>) -> (Self, Self) {
        let BufferSize::Size(at) = at.into() else {
            panic!("Slices cannot have more than isize::MAX elements");
        };
        self.split_at(at)
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
    fn size(&self) -> BufferSize<Self::Idx> {
        BufferSize::Size(self.len())
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

    fn buf_split_at(self, at: impl Into<BufferSize<usize>>) -> (Self, Self) {
        let BufferSize::Size(at) = at.into() else {
            panic!("Slices cannot have more than isize::MAX elements");
        };
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

impl<'a, Idx, B> NarrowedIndexBuffer<'a, Idx, B>
where
    B: Buffer<'a>,
    Idx: NarrowedIndex<B::Idx>,
{
    pub fn new(buffer: B) -> Self {
        assert!(Idx::narrow_size_from(buffer.size()).is_some());
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
    Idx: NarrowedIndex<B::Idx>,
{
    type Idx = Idx;

    fn size(&self) -> BufferSize<Self::Idx> {
        Idx::narrow_size_from(self.buffer.size()).unwrap()
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

    fn buf_split_at(self, at: impl Into<BufferSize<Self::Idx>>) -> (Self, Self) {
        let widened_size = match at.into() {
            BufferSize::Size(size) => BufferSize::Size(size.widen_to()),
            BufferSize::Max => Idx::widened_max_size(),
        };
        let (first, second) = self.buffer.buf_split_at(widened_size);
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
