use std::ops::{Bound, RangeBounds};

use crate::utils::{convert::convert_if_different, errors::NoError};

/// An abstraction over types that contain a buffer of bytes.
///
/// This is designed to be usable with both mutable and immutable byte
/// buffers, and both owned and borrowed buffers.
///
/// Each buffer specifies its own index type, used as a byte offset
/// into the buffer.
pub trait Buffer {
    fn read_slice_at(&self, offset: usize) -> &[u8];
    fn size(&self) -> usize;
}

pub trait SplittableBuffer: Buffer + Sized + Clone {
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

pub trait FallibleBuffer {
    type Error: std::error::Error + Send + Sync + 'static;
    fn read_slice(&self, offset: usize, buf: &mut [u8]) -> Result<(), Self::Error>;
    fn size(&self) -> usize;
}

impl<T: Buffer> FallibleBuffer for T {
    type Error = NoError;

    fn read_slice(&self, offset: usize, mut buf: &mut [u8]) -> Result<(), Self::Error> {
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

pub trait SplittableFallibleBuffer: FallibleBuffer + Sized + Clone {
    fn sub_buffer_from_range(&self, start: usize, end: usize) -> Result<Self, Self::Error>;
}

impl<T: SplittableBuffer> SplittableFallibleBuffer for T {
    fn sub_buffer_from_range(&self, start: usize, end: usize) -> Result<Self, Self::Error> {
        Ok(self.sub_buffer_from_range(start, end))
    }
}

/// A wrapper that implements [`std::io::Read`] for any Buffer.
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
