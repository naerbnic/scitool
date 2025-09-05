use std::ops::{Bound, RangeBounds};

use crate::utils::{convert::convert_if_different, errors::NoError};

/// An abstraction over types that contain a buffer of bytes.
///
/// Allows values to be read from arbitrary offsets.
pub trait Buffer {
    /// Reads a slice starting at the given offset.
    ///
    /// This slice can be of any non-zero length.
    ///
    /// Panics if `offset` is greater than the size of the buffer.
    fn read_slice_at(&self, offset: usize) -> &[u8];

    /// Returns the size of the buffer in bytes.
    fn size(&self) -> usize;
}

/// A buffer whose contents can be extracted as an independent sub-buffer.
pub trait SplittableBuffer: Buffer + Sized + Clone {
    /// Returns a sub-buffer containing the bytes in the given range.
    ///
    /// This will be of the same type as the source object.
    #[must_use]
    fn sub_buffer_from_range(&self, start: usize, end: usize) -> Self;
}

impl Buffer for &[u8] {
    fn read_slice_at(&self, offset: usize) -> &[u8] {
        assert!(offset <= self.len());
        &self[offset..]
    }

    fn size(&self) -> usize {
        self.len()
    }
}

impl SplittableBuffer for &[u8] {
    fn sub_buffer_from_range(&self, start: usize, end: usize) -> Self {
        assert!(start <= end);
        assert!(end <= self.len());
        &self[start..end]
    }
}

/// A buffer that can fail when reading.
pub trait FallibleBuffer {
    type Error: std::error::Error + Send + Sync + 'static;
    /// Reads a slice starting at the given offset into the provided buffer.
    ///
    /// The length of the provided buffer determines how many bytes are read.
    ///
    /// Panics if the end of the read region would be beyond the end of the buffer.
    fn read_slice(&self, offset: usize, buf: &mut [u8]) -> Result<(), Self::Error>;

    /// Returns the size of the buffer in bytes.
    fn size(&self) -> usize;
}

/// All buffers are fallible buffers that never fail.
impl<T: Buffer> FallibleBuffer for T {
    /// The error type is `NoError`, which can never be constructed.
    type Error = NoError;

    fn read_slice(&self, offset: usize, mut buf: &mut [u8]) -> Result<(), Self::Error> {
        assert!(
            offset + buf.len() <= self.size(),
            "Attempted to read beyond end of buffer: offset {offset} + length {} > size {}",
            buf.len(),
            self.size()
        );
        let mut curr_offset = offset;
        while !buf.is_empty() {
            let slice = self.read_slice_at(curr_offset);
            let to_copy = std::cmp::min(slice.len(), buf.len());
            buf[..to_copy].copy_from_slice(&slice[..to_copy]);
            curr_offset += to_copy;
            buf = &mut buf[to_copy..];
        }
        Ok(())
    }

    fn size(&self) -> usize {
        self.size()
    }
}

/// A buffer that can be split and can fail when reading.
pub trait SplittableFallibleBuffer: FallibleBuffer + Sized + Clone {
    fn sub_buffer_from_range(&self, start: usize, end: usize) -> Result<Self, Self::Error>;
}

impl<T: SplittableBuffer> SplittableFallibleBuffer for T {
    fn sub_buffer_from_range(&self, start: usize, end: usize) -> Result<Self, Self::Error> {
        Ok(self.sub_buffer_from_range(start, end))
    }
}

/// A wrapper that implements [`std::io::Read`] and [`bytes::Buf`] for any Buffer.
pub struct BufferCursor<B> {
    buffer: B,
    position: usize,
}

impl<B: FallibleBuffer> BufferCursor<B> {
    pub fn new(buffer: B) -> Self {
        Self {
            buffer,
            position: 0,
        }
    }
}

impl<B: FallibleBuffer> std::io::Read for BufferCursor<B> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let available = std::cmp::min(self.buffer.size() - self.position, buf.len());
        let buf = &mut buf[..available];
        self.buffer
            .read_slice(self.position, buf)
            .map_err(|e| convert_if_different(e, std::io::Error::other))?;
        self.position += buf.len();
        Ok(buf.len())
    }
}

impl<B: Buffer> bytes::Buf for BufferCursor<B> {
    fn remaining(&self) -> usize {
        self.buffer.size() - self.position
    }

    fn chunk(&self) -> &[u8] {
        self.buffer.read_slice_at(self.position)
    }

    fn advance(&mut self, cnt: usize) {
        self.position += cnt;
    }
}

pub trait BufferExt: SplittableFallibleBuffer {
    fn sub_buffer<T, R: RangeBounds<T>>(&self, range: R) -> Result<Self, Self::Error>
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
}

impl<T: SplittableFallibleBuffer> BufferExt for T {}
