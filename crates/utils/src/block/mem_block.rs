use std::{io, ops::RangeBounds, sync::Arc};

use crate::buffer::{Buffer, FromFixedBytes};

use super::{ReadError, ReadResult};

fn get_range_ends<T, R: RangeBounds<T>>(range: R, size: T) -> (T, T)
where
    T: num::Num + Copy,
{
    let start = match range.start_bound() {
        std::ops::Bound::Included(&start) => start,
        std::ops::Bound::Excluded(&start) => start + T::one(),
        std::ops::Bound::Unbounded => T::zero(),
    };

    let end = match range.end_bound() {
        std::ops::Bound::Included(&end) => end + T::one(),
        std::ops::Bound::Excluded(&end) => end,
        std::ops::Bound::Unbounded => size,
    };

    (start, end)
}

/// An in-memory block of data that is cheap to clone, and create subranges of.
#[derive(Clone)]
pub struct MemBlock {
    start: usize,
    size: usize,
    data: Arc<Vec<u8>>,
}

impl MemBlock {
    /// Create the block from a vector of bytes.
    pub fn from_vec(data: Vec<u8>) -> Self {
        let size = data.len();
        Self {
            start: 0,
            size,
            data: Arc::new(data),
        }
    }

    /// Read the entirety of a reader into a block.
    pub fn from_reader<R>(mut reader: R) -> io::Result<Self>
    where
        R: io::Read + io::Seek,
    {
        let size = reader.seek(io::SeekFrom::End(0))?;
        let mut data = vec![0; size.try_into().map_err(ReadError::from_std_err)?];
        reader.seek(io::SeekFrom::Start(0))?;
        reader.read_exact(&mut data)?;
        Ok(Self::from_vec(data))
    }

    /// Returns the size of the block.
    pub fn size(&self) -> usize {
        self.size
    }

    /// Reads a slice of the block into a mutable slice. Returns a read error
    /// if the slice is out of bounds.
    pub fn read_at(&self, offset: usize, buf: &mut [u8]) -> ReadResult<()> {
        if offset + buf.len() > self.size {
            return Err(ReadError::new(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Attempted to read past the end of the block",
            )));
        }

        buf.copy_from_slice(&self.data[self.start + offset..][..buf.len()]);
        Ok(())
    }

    /// Read the entirety of the buffer into a vector.
    pub fn read_all(&self) -> ReadResult<Vec<u8>> {
        let mut buf = vec![0; self.size];
        self.read_at(0, &mut buf)?;
        Ok(buf)
    }

    /// Returns the offset of the contained block within the current block.
    ///
    /// Panics if the argument originated from another block, and is not fully
    /// contained within the current block
    pub fn offset_in(&self, contained_block: &MemBlock) -> usize {
        assert!(Arc::ptr_eq(&self.data, &contained_block.data));
        assert!(self.start <= contained_block.start);
        assert!(contained_block.start + contained_block.size <= self.start + self.size);
        contained_block.start - self.start
    }
}

impl std::ops::Deref for MemBlock {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.data[self.start..][..self.size]
    }
}

impl AsRef<[u8]> for MemBlock {
    fn as_ref(&self) -> &[u8] {
        &self.data[self.start..][..self.size]
    }
}

impl Buffer<'static> for MemBlock {
    type Idx = usize;
    fn size(&self) -> usize {
        self.size
    }

    fn sub_buffer<R: RangeBounds<usize>>(self, range: R) -> Self {
        let (start, end) = get_range_ends(range, self.size);

        // Actual start/end are offsets from self.start
        let start = self.start + start;
        let end = self.start + end;

        assert!(start <= end);
        assert!(
            end <= self.start + self.size,
            "End: {} Size: {}",
            end,
            self.start + self.size
        );

        Self {
            start,
            size: end - start,
            data: self.data,
        }
    }

    fn split_at(self, at: usize) -> (Self, Self) {
        assert!(
            at <= self.size,
            "Tried to split a block of size {} at offset {}",
            self.size,
            at
        );
        (self.clone().sub_buffer(..at), self.sub_buffer(at..))
    }

    fn read_value<T: FromFixedBytes>(self) -> anyhow::Result<(T, Self)> {
        let value_bytes: &[u8] = &self[..T::SIZE];
        let value = T::parse(value_bytes)?;
        let remaining = self.sub_buffer(T::SIZE..);
        Ok((value, remaining))
    }
}

impl std::fmt::Debug for MemBlock {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_tuple("Block")
            .field(&&self.data[self.start..][..self.size])
            .finish()
    }
}

impl bytes::Buf for MemBlock {
    fn remaining(&self) -> usize {
        self.size
    }

    fn chunk(&self) -> &[u8] {
        &self.data[self.start..][..self.size]
    }

    fn advance(&mut self, cnt: usize) {
        assert!(cnt <= self.size);
        self.start += cnt;
        self.size -= cnt;
    }
}
