mod empty_impl;
mod error_impl;
mod mem_impl;
mod read_factory_impl;
mod read_seek_factory_impl;
mod read_seek_impl;
mod seq_impl;

use std::{
    io::{self, Read as _, Seek as _},
    ops::RangeBounds,
    sync::Arc,
};

use crate::utils::{
    block::{
        MemBlock,
        block2::{
            empty_impl::EmptyBlockImpl, error_impl::ErrorBlockImpl, mem_impl::ContainedMemBlock,
            read_factory_impl::ReadFactoryImpl, read_seek_factory_impl::ReadSeekFactorySource,
            read_seek_impl::ReadSeekImpl, seq_impl::SequenceBlockImpl,
        },
    },
    buffer::BufferExt as _,
    range::{BoundedRange, Range},
};

/// Implementation trait for Block sources.
///
/// This is a dyn-compatible trait that provides the core functionality for
/// Block sources.
trait BlockBase {
    // Open as loaded data, possibly shared.
    fn open_mem(&self, range: BoundedRange<u64>) -> io::Result<MemBlock>;

    /// Open as borrowed reader.
    fn open_reader<'a>(&'a self, range: BoundedRange<u64>) -> io::Result<Box<dyn io::Read + 'a>>;
}

trait MemBlockBase {
    fn load_mem_block(&self) -> io::Result<&MemBlock>;
}

trait RangeStreamBase {
    type Reader<'a>: io::Read + 'a
    where
        Self: 'a;
    fn open_range_reader(&self, range: BoundedRange<u64>) -> io::Result<Self::Reader<'_>>;
}

trait FullStreamBase {
    type Reader<'a>: io::Read + 'a
    where
        Self: 'a;
    fn open_full_reader(&self) -> io::Result<Self::Reader<'_>>;
}

struct MemBlockWrap<T>(T);

impl<T> BlockBase for MemBlockWrap<T>
where
    T: MemBlockBase,
{
    fn open_mem(&self, range: BoundedRange<u64>) -> io::Result<MemBlock> {
        let mem_block = self.0.load_mem_block()?;
        let mem_block = mem_block
            .sub_buffer(range.cast_to::<usize>())
            .map_err(io::Error::other)?;
        Ok(mem_block.clone())
    }

    fn open_reader<'a>(&'a self, range: BoundedRange<u64>) -> io::Result<Box<dyn io::Read + 'a>> {
        let mem_block = self.0.load_mem_block()?;
        let mem_block = mem_block
            .sub_buffer(range.cast_to::<usize>())
            .map_err(io::Error::other)?;
        Ok(Box::new(io::Cursor::new(mem_block)))
    }
}

struct RangeStreamBaseWrap<T>(T);

impl<T> BlockBase for RangeStreamBaseWrap<T>
where
    T: RangeStreamBase,
{
    fn open_mem(&self, range: BoundedRange<u64>) -> io::Result<MemBlock> {
        let mut data = Vec::new();
        self.0.open_range_reader(range)?.read_to_end(&mut data)?;
        Ok(MemBlock::from_vec(data))
    }

    fn open_reader<'a>(&'a self, range: BoundedRange<u64>) -> io::Result<Box<dyn io::Read + 'a>> {
        let reader = self.0.open_range_reader(range)?;
        Ok(Box::new(reader))
    }
}

struct FullStreamBaseWrap<T>(T);

impl<T> BlockBase for FullStreamBaseWrap<T>
where
    T: FullStreamBase,
{
    fn open_mem(&self, range: BoundedRange<u64>) -> io::Result<MemBlock> {
        let mut data = Vec::new();
        self.open_reader(range)?.read_to_end(&mut data)?;
        Ok(MemBlock::from_vec(data))
    }

    fn open_reader<'a>(&'a self, range: BoundedRange<u64>) -> io::Result<Box<dyn io::Read + 'a>> {
        let mut reader = self.0.open_full_reader()?;
        let temp_buffer = &mut [0u8; 8192];
        let mut data_remaining = range.start();
        while data_remaining > 0 {
            let to_read = std::cmp::min(data_remaining, temp_buffer.len() as u64);
            let read_bytes = reader.read(&mut temp_buffer[..to_read.try_into().unwrap()])?;
            if read_bytes == 0 {
                break;
            }
            data_remaining -= read_bytes as u64;
        }
        Ok(Box::new(reader.take(range.size())))
    }
}

/// A helper trait for creating objects that borrow from the factory.
pub trait RefFactory {
    type Output<'a>
    where
        Self: 'a;

    fn create_new(&self) -> io::Result<Self::Output<'_>>;
}

impl<F, T> RefFactory for F
where
    F: Fn() -> io::Result<T>,
{
    type Output<'a>
        = T
    where
        Self: 'a;

    fn create_new(&self) -> io::Result<Self::Output<'_>> {
        self()
    }
}

#[derive(Clone)]
pub struct Block {
    source: Arc<dyn BlockBase + Send + Sync>,
    range: BoundedRange<u64>,
}

impl Block {
    /// Private constructor taking an explicit source and range.
    fn from_source_size<B>(source: B, size: u64) -> Self
    where
        B: BlockBase + Send + Sync + 'static,
    {
        Self {
            source: Arc::new(source),
            range: BoundedRange::from_size(size),
        }
    }

    /// Returns an empty block.
    #[must_use]
    pub fn empty() -> Self {
        Self::from_source_size(EmptyBlockImpl, 0)
    }

    /// Returns a block that always errors on access.
    #[must_use]
    pub fn from_error<E>(error: E) -> Self
    where
        E: Into<io::Error> + Clone + Send + Sync + 'static,
    {
        let source = ErrorBlockImpl::new(error);
        Self::from_source_size(source, 0)
    }

    /// Create a block from a [`MemBlock`] instance.
    #[must_use]
    pub fn from_mem_block(mem_block: MemBlock) -> Self {
        let len = mem_block.len() as u64;
        Self::from_source_size(MemBlockWrap(ContainedMemBlock::new(mem_block)), len)
    }

    /// Create a block by concatenating multiple blocks together.
    #[must_use]
    pub fn concat_blocks(blocks: impl IntoIterator<Item = impl Into<Block>>) -> Self {
        let base_impl = SequenceBlockImpl::new(blocks.into_iter().map(Into::into));
        let total_size = base_impl.size();
        Self::from_source_size(base_impl, total_size)
    }

    /// Returns a new builder for creating Block instances.
    #[must_use]
    pub fn builder() -> Builder {
        Builder::new()
    }

    /// Open a subrange of the block as loaded data.
    pub fn open_mem<R>(&self, range: R) -> io::Result<MemBlock>
    where
        R: RangeBounds<u64>,
    {
        let range = Range::from_range(range);
        self.source.open_mem(self.range.new_relative(range))
    }

    /// Open a subrange of the block as a reader.
    pub fn open_reader<'a, R>(&'a self, range: R) -> io::Result<Box<dyn io::Read + 'a>>
    where
        R: RangeBounds<u64>,
    {
        let range = Range::from_range(range);
        self.source.open_reader(self.range.new_relative(range))
    }

    /// Returns the length of the block in bytes.
    #[must_use]
    pub fn len(&self) -> u64 {
        self.range.size()
    }

    /// Returns whether the block is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns a sub-block source that represents a subrange of the current
    /// block source.
    #[must_use]
    pub fn subblock<R>(&self, range: R) -> Self
    where
        R: RangeBounds<u64>,
    {
        let range = Range::from_range(range);

        Self {
            source: self.source.clone(),
            range: self.range.new_relative(range),
        }
    }
}

impl From<MemBlock> for Block {
    fn from(mem_block: MemBlock) -> Self {
        Self::from_mem_block(mem_block)
    }
}

/// A builder for Block instances
///
/// This allows blocks to be created from multiple settings, including being
/// able to be given an explicit initial size.
pub struct Builder {
    size: Option<u64>,
}

impl Builder {
    #[must_use]
    pub fn new() -> Self {
        Self { size: None }
    }

    #[must_use]
    pub fn with_size(mut self, size: u64) -> Self {
        self.size = Some(size);
        self
    }

    pub fn build_from_read_seek(
        self,
        mut reader: impl io::Read + io::Seek + Send + 'static,
    ) -> io::Result<Block> {
        let size = if let Some(size) = self.size {
            size
        } else {
            reader.seek(io::SeekFrom::End(0))?
        };

        Ok(Block::from_source_size(
            RangeStreamBaseWrap(ReadSeekImpl::new(reader)),
            size,
        ))
    }

    pub fn build_from_read_factory<F>(self, factory: F) -> io::Result<Block>
    where
        F: RefFactory + Send + Sync + 'static,
        for<'a> F::Output<'a>: io::Read,
    {
        let size = if let Some(size) = self.size {
            size
        } else {
            // Count size by reading all data
            io::copy(&mut factory.create_new()?, &mut io::sink())?
        };
        Ok(Block::from_source_size(
            FullStreamBaseWrap(ReadFactoryImpl::new(factory)),
            size,
        ))
    }

    pub fn build_from_read_seek_factory<F>(self, factory: F) -> io::Result<Block>
    where
        F: RefFactory + Send + Sync + 'static,
        for<'a> F::Output<'a>: io::Read + io::Seek,
    {
        let size = if let Some(size) = self.size {
            size
        } else {
            let mut reader = factory.create_new()?;
            reader.seek(io::SeekFrom::End(0))?
        };

        Ok(Block::from_source_size(
            RangeStreamBaseWrap(ReadSeekFactorySource::new(factory)),
            size,
        ))
    }
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Read};

    #[test]
    fn test_empty_block() {
        let block = Block::empty();
        assert_eq!(block.len(), 0);
        assert!(block.is_empty());
    }

    #[test]
    fn test_empty_block_open_mem() {
        let block = Block::empty();
        let mem = block.open_mem(..).unwrap();
        assert_eq!(mem.len(), 0);
    }

    #[test]
    fn test_empty_block_open_reader() {
        let block = Block::empty();
        let mut reader = block.open_reader(..).unwrap();
        let mut data = Vec::new();
        reader.read_to_end(&mut data).unwrap();
        assert_eq!(data.len(), 0);
    }

    #[test]
    fn test_from_error_block() {
        let block = Block::from_error(io::ErrorKind::Other);
        assert_eq!(block.len(), 0);
    }

    #[test]
    fn test_from_error_block_open_mem_fails() {
        let block = Block::from_error(io::ErrorKind::Other);
        let result = block.open_mem(..);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_error_block_open_reader_fails() {
        let block = Block::from_error(io::ErrorKind::Other);
        let result = block.open_reader(..);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_mem_block() {
        let data = vec![1, 2, 3, 4, 5];
        let mem_block = MemBlock::from_vec(data.clone());
        let block = Block::from_mem_block(mem_block);
        assert_eq!(block.len(), 5);
        assert!(!block.is_empty());
    }

    #[test]
    fn test_from_mem_block_open_mem() {
        let data = vec![1, 2, 3, 4, 5];
        let mem_block = MemBlock::from_vec(data.clone());
        let block = Block::from_mem_block(mem_block);
        let mem = block.open_mem(..).unwrap();
        assert_eq!(mem.as_ref(), &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_from_mem_block_open_reader() {
        let data = vec![1, 2, 3, 4, 5];
        let mem_block = MemBlock::from_vec(data.clone());
        let block = Block::from_mem_block(mem_block);
        let mut reader = block.open_reader(..).unwrap();
        let mut result = Vec::new();
        reader.read_to_end(&mut result).unwrap();
        assert_eq!(result, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_from_trait() {
        let data = vec![1, 2, 3, 4, 5];
        let mem_block = MemBlock::from_vec(data);
        let block: Block = mem_block.into();
        assert_eq!(block.len(), 5);
    }

    #[test]
    fn test_open_mem_with_range() {
        let data = vec![1, 2, 3, 4, 5];
        let mem_block = MemBlock::from_vec(data);
        let block = Block::from_mem_block(mem_block);
        let mem = block.open_mem(1..4).unwrap();
        assert_eq!(mem.as_ref(), &[2, 3, 4]);
    }

    #[test]
    fn test_open_mem_with_start_range() {
        let data = vec![1, 2, 3, 4, 5];
        let mem_block = MemBlock::from_vec(data);
        let block = Block::from_mem_block(mem_block);
        let mem = block.open_mem(2..).unwrap();
        assert_eq!(mem.as_ref(), &[3, 4, 5]);
    }

    #[test]
    fn test_open_mem_with_end_range() {
        let data = vec![1, 2, 3, 4, 5];
        let mem_block = MemBlock::from_vec(data);
        let block = Block::from_mem_block(mem_block);
        let mem = block.open_mem(..3).unwrap();
        assert_eq!(mem.as_ref(), &[1, 2, 3]);
    }

    #[test]
    fn test_open_reader_with_range() {
        let data = vec![1, 2, 3, 4, 5];
        let mem_block = MemBlock::from_vec(data);
        let block = Block::from_mem_block(mem_block);
        let mut reader = block.open_reader(1..4).unwrap();
        let mut result = Vec::new();
        reader.read_to_end(&mut result).unwrap();
        assert_eq!(result, vec![2, 3, 4]);
    }

    #[test]
    fn test_open_reader_with_start_range() {
        let data = vec![1, 2, 3, 4, 5];
        let mem_block = MemBlock::from_vec(data);
        let block = Block::from_mem_block(mem_block);
        let mut reader = block.open_reader(2..).unwrap();
        let mut result = Vec::new();
        reader.read_to_end(&mut result).unwrap();
        assert_eq!(result, vec![3, 4, 5]);
    }

    #[test]
    fn test_open_reader_with_end_range() {
        let data = vec![1, 2, 3, 4, 5];
        let mem_block = MemBlock::from_vec(data);
        let block = Block::from_mem_block(mem_block);
        let mut reader = block.open_reader(..3).unwrap();
        let mut result = Vec::new();
        reader.read_to_end(&mut result).unwrap();
        assert_eq!(result, vec![1, 2, 3]);
    }

    #[test]
    fn test_subblock() {
        let data = vec![1, 2, 3, 4, 5];
        let mem_block = MemBlock::from_vec(data);
        let block = Block::from_mem_block(mem_block);
        let subblock = block.subblock(1..4);
        assert_eq!(subblock.len(), 3);
        let mem = subblock.open_mem(..).unwrap();
        assert_eq!(mem.as_ref(), &[2, 3, 4]);
    }

    #[test]
    fn test_subblock_of_subblock() {
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let mem_block = MemBlock::from_vec(data);
        let block = Block::from_mem_block(mem_block);
        let subblock1 = block.subblock(1..7);
        let subblock2 = subblock1.subblock(1..4);
        assert_eq!(subblock2.len(), 3);
        let mem = subblock2.open_mem(..).unwrap();
        assert_eq!(mem.as_ref(), &[3, 4, 5]);
    }

    #[test]
    fn test_concat_blocks_empty() {
        let blocks: Vec<Block> = vec![];
        let concatenated = Block::concat_blocks(blocks);
        assert_eq!(concatenated.len(), 0);
        assert!(concatenated.is_empty());
    }

    #[test]
    fn test_concat_blocks_single() {
        let data = vec![1, 2, 3];
        let mem_block = MemBlock::from_vec(data);
        let block = Block::from_mem_block(mem_block);
        let concatenated = Block::concat_blocks(vec![block]);
        assert_eq!(concatenated.len(), 3);
        let mem = concatenated.open_mem(..).unwrap();
        assert_eq!(mem.as_ref(), &[1, 2, 3]);
    }

    #[test]
    fn test_concat_blocks_multiple() {
        let block1 = MemBlock::from_vec(vec![1, 2]);
        let block2 = MemBlock::from_vec(vec![3, 4]);
        let block3 = MemBlock::from_vec(vec![5, 6]);
        let concatenated = Block::concat_blocks(vec![block1, block2, block3]);
        assert_eq!(concatenated.len(), 6);
        let mem = concatenated.open_mem(..).unwrap();
        assert_eq!(mem.as_ref(), &[1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn test_concat_blocks_with_empty() {
        let block1 = Block::from_mem_block(MemBlock::from_vec(vec![1, 2]));
        let block2 = Block::empty();
        let block3 = Block::from_mem_block(MemBlock::from_vec(vec![3, 4]));
        let concatenated = Block::concat_blocks(vec![block1, block2, block3]);
        assert_eq!(concatenated.len(), 4);
        let mem = concatenated.open_mem(..).unwrap();
        assert_eq!(mem.as_ref(), &[1, 2, 3, 4]);
    }

    #[test]
    fn test_concat_blocks_open_reader() {
        let block1 = Block::from_mem_block(MemBlock::from_vec(vec![1, 2]));
        let block2 = Block::from_mem_block(MemBlock::from_vec(vec![3, 4]));
        let concatenated = Block::concat_blocks(vec![block1, block2]);
        let mut reader = concatenated.open_reader(..).unwrap();
        let mut result = Vec::new();
        reader.read_to_end(&mut result).unwrap();
        assert_eq!(result, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_concat_blocks_subblock() {
        let block1 = Block::from_mem_block(MemBlock::from_vec(vec![1, 2]));
        let block2 = Block::from_mem_block(MemBlock::from_vec(vec![3, 4]));
        let block3 = Block::from_mem_block(MemBlock::from_vec(vec![5, 6]));
        let concatenated = Block::concat_blocks(vec![block1, block2, block3]);
        let subblock = concatenated.subblock(1..5);
        assert_eq!(subblock.len(), 4);
        let mem = subblock.open_mem(..).unwrap();
        assert_eq!(mem.as_ref(), &[2, 3, 4, 5]);
    }

    #[test]
    fn test_builder_new() {
        let builder = Builder::new();
        assert!(builder.size.is_none());
    }

    #[test]
    fn test_builder_default() {
        let builder = Builder::default();
        assert!(builder.size.is_none());
    }

    #[test]
    fn test_builder_with_size() {
        let builder = Builder::new().with_size(100);
        assert_eq!(builder.size, Some(100));
    }

    #[test]
    fn test_builder_from_read_seek() {
        let data = vec![1, 2, 3, 4, 5];
        let cursor = Cursor::new(data.clone());
        let block = Builder::new().build_from_read_seek(cursor).unwrap();
        assert_eq!(block.len(), 5);
        let mem = block.open_mem(..).unwrap();
        assert_eq!(mem.as_ref(), &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_builder_from_read_seek_with_explicit_size() {
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let cursor = Cursor::new(data);
        let block = Builder::new()
            .with_size(5)
            .build_from_read_seek(cursor)
            .unwrap();
        assert_eq!(block.len(), 5);
        let mem = block.open_mem(..).unwrap();
        assert_eq!(mem.as_ref(), &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_builder_from_read_factory() {
        let data = vec![1, 2, 3, 4, 5];
        let factory = move || Ok(Cursor::new(data.clone()));
        let block = Builder::new().build_from_read_factory(factory).unwrap();
        assert_eq!(block.len(), 5);
        let mem = block.open_mem(..).unwrap();
        assert_eq!(mem.as_ref(), &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_builder_from_read_factory_with_explicit_size() {
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let factory = move || Ok(Cursor::new(data.clone()));
        let block = Builder::new()
            .with_size(5)
            .build_from_read_factory(factory)
            .unwrap();
        assert_eq!(block.len(), 5);
        let mem = block.open_mem(..).unwrap();
        assert_eq!(mem.as_ref(), &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_builder_from_read_seek_factory() {
        let data = vec![1, 2, 3, 4, 5];
        let factory = move || Ok(Cursor::new(data.clone()));
        let block = Builder::new()
            .build_from_read_seek_factory(factory)
            .unwrap();
        assert_eq!(block.len(), 5);
        let mem = block.open_mem(..).unwrap();
        assert_eq!(mem.as_ref(), &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_builder_from_read_seek_factory_with_explicit_size() {
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let factory = move || Ok(Cursor::new(data.clone()));
        let block = Builder::new()
            .with_size(5)
            .build_from_read_seek_factory(factory)
            .unwrap();
        assert_eq!(block.len(), 5);
        let mem = block.open_mem(..).unwrap();
        assert_eq!(mem.as_ref(), &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_builder_factory_multiple_calls() {
        let data = vec![1, 2, 3, 4, 5];
        let factory = move || Ok(Cursor::new(data.clone()));
        let block = Builder::new()
            .build_from_read_seek_factory(factory)
            .unwrap();

        // Multiple calls to open_mem should work
        let mem1 = block.open_mem(..).unwrap();
        let mem2 = block.open_mem(..).unwrap();
        assert_eq!(mem1.as_ref(), &[1, 2, 3, 4, 5]);
        assert_eq!(mem2.as_ref(), &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_block_clone() {
        let data = vec![1, 2, 3, 4, 5];
        let mem_block = MemBlock::from_vec(data);
        let block = Block::from_mem_block(mem_block);
        let cloned = block.clone();

        assert_eq!(block.len(), cloned.len());
        let mem1 = block.open_mem(..).unwrap();
        let mem2 = cloned.open_mem(..).unwrap();
        assert_eq!(mem1.as_ref(), mem2.as_ref());
    }

    #[test]
    fn test_open_mem_subrange_of_read_seek() {
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let cursor = Cursor::new(data);
        let block = Builder::new().build_from_read_seek(cursor).unwrap();
        let mem = block.open_mem(2..6).unwrap();
        assert_eq!(mem.as_ref(), &[3, 4, 5, 6]);
    }

    #[test]
    fn test_open_reader_subrange_of_read_seek() {
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let cursor = Cursor::new(data);
        let block = Builder::new().build_from_read_seek(cursor).unwrap();
        let mut reader = block.open_reader(2..6).unwrap();
        let mut result = Vec::new();
        reader.read_to_end(&mut result).unwrap();
        assert_eq!(result, vec![3, 4, 5, 6]);
    }
}
