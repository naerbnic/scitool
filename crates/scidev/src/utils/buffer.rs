use std::{
    io,
    ops::RangeBounds,
    sync::{Arc, Mutex},
};

use crate::utils::{
    convert::convert_if_different,
    mem_reader::FromFixedBytes,
    range::{BoundedRange, Range},
};

pub trait SizedData {
    /// Returns the size of the buffer in bytes.
    fn size(&self) -> usize;
}

impl<B> SizedData for &B
where
    B: SizedData + ?Sized,
{
    fn size(&self) -> usize {
        (*self).size()
    }
}

impl<B> SizedData for &mut B
where
    B: SizedData + ?Sized,
{
    fn size(&self) -> usize {
        (**self).size()
    }
}

impl<B> SizedData for Arc<B>
where
    B: SizedData,
{
    fn size(&self) -> usize {
        (**self).size()
    }
}

impl<B> SizedData for Mutex<B>
where
    B: SizedData,
{
    fn size(&self) -> usize {
        self.lock().unwrap().size()
    }
}

impl SizedData for [u8] {
    fn size(&self) -> usize {
        self.len()
    }
}

/// An abstraction over types that contain a buffer of bytes.
///
/// Allows values to be read from arbitrary offsets.
pub trait Buffer: SizedData {
    /// Reads a slice starting at the given offset.
    ///
    /// This slice can be of any non-zero length.
    ///
    /// Panics if `offset` is greater than the size of the buffer.
    fn read_slice_at(&self, offset: usize) -> &[u8];

    fn into_fallible(self) -> FallibleBufWrap<Self>
    where
        Self: Sized,
    {
        FallibleBufWrap { buffer: self }
    }

    fn as_fallible(&self) -> FallibleBufWrap<&Self>
    where
        Self: Sized,
    {
        FallibleBufWrap { buffer: self }
    }
}

impl<B> Buffer for &B
where
    B: Buffer,
{
    fn read_slice_at(&self, offset: usize) -> &[u8] {
        (*self).read_slice_at(offset)
    }
}

pub trait Splittable: SizedData + Sized {
    /// Returns a sub-buffer containing the bytes in the given range.
    ///
    /// This will be of the same type as the source object.
    #[must_use]
    fn sub_buffer_from_range(&self, range: BoundedRange<usize>) -> Self;

    #[must_use]
    fn sub_buffer<T, R: RangeBounds<T>>(&self, range: R) -> Self
    where
        T: num::PrimInt + num::Unsigned + Into<usize> + 'static,
    {
        let range = Range::from_range(range);
        let self_range = BoundedRange::from_size(self.size()).new_relative(range.coerce_to());

        self.sub_buffer_from_range(self_range)
    }
}

/// A buffer whose contents can be extracted as an independent sub-buffer.
pub trait SplittableBuffer: Splittable + Buffer {}

impl<B> SplittableBuffer for B where B: Splittable + Buffer {}

impl Buffer for &[u8] {
    fn read_slice_at(&self, offset: usize) -> &[u8] {
        assert!(offset <= self.len());
        &self[offset..]
    }
}

#[derive(Debug)]
pub struct BufferRef<'a, T>(&'a T);

impl<'a, T> From<&'a T> for BufferRef<'a, T> {
    fn from(value: &'a T) -> Self {
        BufferRef(value)
    }
}

impl<T> Clone for BufferRef<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for BufferRef<'_, T> {}

impl<T> SizedData for BufferRef<'_, T>
where
    T: SizedData,
{
    fn size(&self) -> usize {
        self.0.size()
    }
}

impl<T> Buffer for BufferRef<'_, T>
where
    T: Buffer,
{
    fn read_slice_at(&self, offset: usize) -> &[u8] {
        self.0.read_slice_at(offset)
    }
}

impl Splittable for &[u8] {
    fn sub_buffer_from_range(&self, range: BoundedRange<usize>) -> Self {
        &self[range.start()..range.end()]
    }
}

/// A buffer that can fail when reading.
pub trait FallibleBuffer: SizedData {
    /// Reads a slice starting at the given offset into the provided buffer.
    ///
    /// The length of the provided buffer determines how many bytes are read.
    ///
    /// Panics if the end of the read region would be beyond the end of the buffer.
    fn read_slice(&self, offset: usize, buf: &mut [u8]) -> io::Result<()>;
}

impl FallibleBuffer for [u8] {
    fn read_slice(&self, offset: usize, buf: &mut [u8]) -> io::Result<()> {
        buf.copy_from_slice(&self[offset..][..buf.len()]);
        Ok(())
    }
}

impl<T> FallibleBuffer for &T
where
    T: FallibleBuffer + ?Sized,
{
    fn read_slice(&self, offset: usize, buf: &mut [u8]) -> io::Result<()> {
        (*self).read_slice(offset, buf)
    }
}

impl<T> FallibleBuffer for &mut T
where
    T: FallibleBuffer + ?Sized,
{
    fn read_slice(&self, offset: usize, buf: &mut [u8]) -> io::Result<()> {
        (**self).read_slice(offset, buf)
    }
}

impl<T> FallibleBuffer for Arc<T>
where
    T: FallibleBuffer,
{
    fn read_slice(&self, offset: usize, buf: &mut [u8]) -> io::Result<()> {
        (**self).read_slice(offset, buf)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FallibleBufWrap<B> {
    buffer: B,
}

impl<B> SizedData for FallibleBufWrap<B>
where
    B: SizedData,
{
    fn size(&self) -> usize {
        self.buffer.size()
    }
}

impl<B> FallibleBuffer for FallibleBufWrap<B>
where
    B: Buffer,
{
    fn read_slice(&self, offset: usize, buf: &mut [u8]) -> io::Result<()> {
        assert!(
            offset + buf.len() <= self.buffer.size(),
            "Attempted to read beyond end of buffer: offset {offset} + length {} > size {}",
            buf.len(),
            self.buffer.size()
        );
        let mut curr_offset = offset;
        let mut buf = buf;
        while !buf.is_empty() {
            let slice = self.buffer.read_slice_at(curr_offset);
            let to_copy = std::cmp::min(slice.len(), buf.len());
            buf[..to_copy].copy_from_slice(&slice[..to_copy]);
            curr_offset += to_copy;
            buf = &mut buf[to_copy..];
        }
        Ok(())
    }
}

impl<B> Splittable for FallibleBufWrap<B>
where
    B: SplittableBuffer + Clone,
{
    fn sub_buffer_from_range(&self, range: BoundedRange<usize>) -> Self {
        self.buffer.sub_buffer_from_range(range).into_fallible()
    }
}

#[derive(Debug)]
pub struct ReaderBuffer<R> {
    reader: Mutex<R>,
    size: usize,
}

impl<R> ReaderBuffer<R>
where
    R: io::Read + io::Seek,
{
    pub fn new(mut reader: R) -> io::Result<Self> {
        let size = reader.seek(io::SeekFrom::End(0))?;
        let size = usize::try_from(size).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Buffer size exceeds addressable range",
            )
        })?;
        Ok(Self {
            reader: Mutex::new(reader),
            size,
        })
    }

    pub fn try_into_inner(self) -> R {
        self.reader.into_inner().unwrap()
    }
}

impl<R> SizedData for ReaderBuffer<R>
where
    R: io::Read + io::Seek,
{
    fn size(&self) -> usize {
        self.size
    }
}

impl<R> FallibleBuffer for ReaderBuffer<R>
where
    R: io::Read + io::Seek,
{
    fn read_slice(&self, offset: usize, buf: &mut [u8]) -> io::Result<()> {
        assert!(
            offset + buf.len() <= self.size,
            "Attempted to read beyond end of buffer: offset {offset} + length {} > size {}",
            buf.len(),
            self.size
        );
        let mut reader = self.reader.lock().unwrap();
        reader.seek(io::SeekFrom::Start(offset as u64))?;
        reader.read_exact(buf)?;
        Ok(())
    }
}

pub struct FallibleBufferRef<'a, T>(&'a T)
where
    T: ?Sized;
impl<'a, T> From<&'a T> for FallibleBufferRef<'a, T> {
    fn from(value: &'a T) -> Self {
        FallibleBufferRef(value)
    }
}
impl<T> Clone for FallibleBufferRef<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for FallibleBufferRef<'_, T> {}

impl<T> SizedData for FallibleBufferRef<'_, T>
where
    T: SizedData,
{
    fn size(&self) -> usize {
        self.0.size()
    }
}

impl<T> Buffer for FallibleBufferRef<'_, T>
where
    T: Buffer,
{
    fn read_slice_at(&self, offset: usize) -> &[u8] {
        self.0.read_slice_at(offset)
    }
}

impl<T> FallibleBuffer for FallibleBufferRef<'_, T>
where
    T: FallibleBuffer,
{
    fn read_slice(&self, offset: usize, buf: &mut [u8]) -> io::Result<()> {
        self.0.read_slice(offset, buf)
    }
}

/// A buffer that can be split and can fail when reading.
pub trait SplittableFallibleBuffer: Splittable + FallibleBuffer {}
impl<T> SplittableFallibleBuffer for T where T: Splittable + FallibleBuffer {}

/// A wrapper that implements [`std::io::Read`] and [`bytes::Buf`] for any Buffer.
pub struct BufferCursor<B> {
    buffer: B,
    position: usize,
}

impl<B> BufferCursor<B> {
    pub fn new(buffer: B) -> Self {
        Self {
            buffer,
            position: 0,
        }
    }
}

impl<B: FallibleBuffer> io::Read for BufferCursor<B> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let available = std::cmp::min(self.buffer.size() - self.position, buf.len());
        let buf = &mut buf[..available];
        self.buffer
            .read_slice(self.position, buf)
            .map_err(|e| convert_if_different(e, io::Error::other))?;
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

pub trait BufferExt: Buffer {
    fn read_at<T: FromFixedBytes>(&self, offset: usize) -> T {
        assert!(offset + T::SIZE <= self.size());
        let read_slice = self.read_slice_at(offset);
        if read_slice.len() < T::SIZE {
            // The bytes are not contiguous, so we need to copy them
            let mut buf = Vec::with_capacity(T::SIZE);
            buf.extend_from_slice(read_slice);
            while buf.len() < T::SIZE {
                let next_slice = self.read_slice_at(offset + buf.len());
                buf.extend_from_slice(&next_slice[..(T::SIZE - buf.len())]);
            }
            T::parse(&*buf)
        } else {
            T::parse(&read_slice[..T::SIZE])
        }
    }
}

impl<B: Buffer> BufferExt for B {}
