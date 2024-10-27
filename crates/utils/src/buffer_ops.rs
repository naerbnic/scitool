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

/// An abstraction over types that contain a buffer of bytes.
///
/// This is designed to be usable with both mutable and immutable byte
/// buffers, and both owned and borrowed buffers.
pub trait Buffer<'a>: Sized + std::ops::Deref
where
    <Self as std::ops::Deref>::Target: AsRef<[u8]>,
{
    fn sub_buffer<R: RangeBounds<usize>>(self, range: R) -> Self;
    fn buf_split_at(self, at: usize) -> (Self, Self);
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
}

impl<'a> Buffer<'a> for &'a [u8] {
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

    fn buf_split_at(self, at: usize) -> (Self, Self) {
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

    fn buf_split_at(self, at: usize) -> (Self, Self) {
        self.split_at_mut(at)
    }

    fn read_value<T: FromFixedBytes>(self) -> anyhow::Result<(T, Self)> {
        let (first, second) = self.split_at_mut(T::SIZE);
        T::parse(first).map(|value| (value, second))
    }
}
