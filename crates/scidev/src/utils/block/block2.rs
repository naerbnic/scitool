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
            mem_impl::ContainedMemBlock, read_factory_impl::ReadFactoryImpl,
            read_seek_factory_impl::ReadSeekFactorySource, read_seek_impl::ReadSeekImpl,
            seq_impl::SequenceBlockImpl,
        },
    },
    buffer::BufferExt as _,
    range::{BoundedRange, Range},
};

pub type ReadScopeFn<'a> = dyn FnMut(&mut dyn io::Read) -> io::Result<()> + Send + 'a;

trait BlockBase {
    // Features we want to support:
    //
    // - A block that needs to be read from disk at a known offset/length.
    // - A block that is generated on demand, possibly from other blocks.
    // - Splitting of blocks into sub-blocks, without the data having to be
    //   loaded first.
    // - Caching of loaded blocks.
    // - Sharing data with other blocks (e.g., via reference counting).
    //
    // Constraints:
    //
    // - Data in a block is considered stable, so opening it multiple times
    //   should yield the same data (or cause an error).
    //
    // This implies we have to have the following capabilities:
    //
    // - Open a full block contents (size unknown).
    // - Open a sub-range of a block (when size known).
    // - Read the contents of a block within a range (when size known).
    // - Read all of the contents of a block (size unknown) while being able to
    //   stop before the end.

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

    #[must_use]
    pub fn from_mem_block(mem_block: MemBlock) -> Self {
        let len = mem_block.len() as u64;
        Self::from_source_size(MemBlockWrap(ContainedMemBlock::new(mem_block)), len)
    }

    #[must_use]
    pub fn concat_blocks(blocks: impl IntoIterator<Item = Block>) -> Self {
        let base_impl = SequenceBlockImpl::new(blocks);
        let total_size = base_impl.size();
        Self::from_source_size(base_impl, total_size)
    }

    #[must_use]
    pub fn builder() -> Builder {
        Builder::new()
    }

    pub fn open_mem<R>(&self, range: R) -> io::Result<MemBlock>
    where
        R: RangeBounds<u64>,
    {
        let range = Range::from_range(range);
        self.source.open_mem(self.range.new_relative(range))
    }

    pub fn open_reader<'a, R>(&'a self, range: R) -> io::Result<Box<dyn io::Read + 'a>>
    where
        R: RangeBounds<u64>,
    {
        let range = Range::from_range(range);
        self.source.open_reader(self.range.new_relative(range))
    }

    #[must_use]
    pub fn len(&self) -> u64 {
        self.range.size()
    }

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
