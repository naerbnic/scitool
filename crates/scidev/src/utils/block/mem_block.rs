use std::{io, sync::Arc};

use bytes::BufMut;

use crate::utils::buffer::{Buffer, SplittableBuffer};

#[derive(Debug, thiserror::Error)]
pub enum FromReaderError {
    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    Conversion(#[from] std::num::TryFromIntError),
}

/// An in-memory block of data that is cheap to clone and create subranges of.
#[derive(Clone)]
pub struct MemBlock {
    start: usize,
    size: usize,
    data: Arc<dyn AsRef<[u8]> + Send + Sync>,
}

impl MemBlock {
    /// Create the block from a vector of bytes.
    #[must_use]
    pub fn from_vec(data: Vec<u8>) -> Self {
        Self::from_slice_owner(data.into_boxed_slice())
    }

    #[must_use]
    pub fn concat_blocks(blocks: impl IntoIterator<Item = MemBlock>) -> Self {
        // There's a potential optimization here, if we can detect that all
        // blocks are from the same underlying data, we can avoid copying. For
        // now, we always copy.
        let mut data = Vec::new();
        for block in blocks {
            data.extend_from_slice(&block);
        }
        Self::from_vec(data)
    }

    pub fn from_slice_owner<T: AsRef<[u8]> + Send + Sync + 'static>(data: T) -> Self {
        let size = data.as_ref().len();
        Self {
            start: 0,
            size,
            data: Arc::new(data),
        }
    }

    pub fn from_buf<B>(buf: B) -> Self
    where
        B: bytes::Buf,
    {
        let size = buf.remaining();
        let mut data = Vec::with_capacity(size);
        data.put(buf);
        Self {
            start: 0,
            size,
            data: Arc::new(data),
        }
    }

    /// Read the entirety of a reader into a block.
    pub fn from_reader<R>(mut reader: R) -> Result<Self, FromReaderError>
    where
        R: io::Read + io::Seek,
    {
        let size = reader.seek(io::SeekFrom::End(0))?;
        let mut data = vec![0; size.try_into()?];
        reader.seek(io::SeekFrom::Start(0))?;
        reader.read_exact(&mut data)?;
        Ok(Self::from_vec(data))
    }

    #[must_use]
    pub fn read_all(&self) -> Vec<u8> {
        self.to_vec()
    }

    /// Returns the size of the block.
    #[must_use]
    pub fn size(&self) -> usize {
        self.size
    }

    /// Returns the offset of the contained block within the current block.
    ///
    /// Panics if the argument originated from another block, and is not fully
    /// contained within the current block
    #[must_use]
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
        &(*self.data).as_ref()[self.start..][..self.size]
    }
}

impl AsRef<[u8]> for MemBlock {
    fn as_ref(&self) -> &[u8] {
        &(*self.data).as_ref()[self.start..][..self.size]
    }
}

impl Buffer for MemBlock {
    fn read_slice_at(&self, offset: usize) -> &[u8] {
        assert!(offset <= self.size);
        &self[offset..]
    }

    fn size(&self) -> usize {
        self.size
    }
}

impl SplittableBuffer for MemBlock {
    fn sub_buffer_from_range(&self, start: usize, end: usize) -> Self {
        let start: usize = start;
        let end: usize = end;

        assert!(start <= end);

        // Actual start/end are offsets from self.start
        let start = self.start + start;
        let end = self.start + end;

        Self {
            start,
            size: end - start,
            data: self.data.clone(),
        }
    }
}

impl std::fmt::Debug for MemBlock {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_tuple("Block").field(&&self[..]).finish()
    }
}
