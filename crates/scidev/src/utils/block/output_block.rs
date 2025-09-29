use std::{io::Write, sync::Arc};

use bytes::Buf;

use crate::utils::{
    block::BlockSource,
    buffer::{Buffer, BufferCursor, SplittableBuffer},
    errors::{OtherError, ensure_other, prelude::*},
};

pub struct BlockData<'a>(Box<dyn bytes::Buf + 'a>);

impl<'a> BlockData<'a> {
    pub fn new<T>(buf: T) -> Self
    where
        T: bytes::Buf + 'a,
    {
        Self(Box::new(buf))
    }

    pub fn from_buffer<B>(buf: B) -> Self
    where
        B: Buffer + 'a,
    {
        Self(Box::new(BufferCursor::new(buf)))
    }
}

impl Buf for BlockData<'_> {
    fn remaining(&self) -> usize {
        self.0.remaining()
    }

    fn chunk(&self) -> &[u8] {
        self.0.chunk()
    }

    fn advance(&mut self, cnt: usize) {
        self.0.advance(cnt);
    }
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum OutputBlockError {
    #[doc(hidden)]
    #[error(transparent)]
    ReadError(#[from] OtherError),
}

type BufIter<'a> = Box<dyn Iterator<Item = Result<BlockData<'a>, OutputBlockError>> + 'a>;

trait OutputBlockImpl: Send + Sync {
    fn size(&self) -> u64;
    fn blocks(&self) -> BufIter<'_>;
}

impl<T> OutputBlockImpl for Box<T>
where
    T: OutputBlockImpl,
{
    fn size(&self) -> u64 {
        self.as_ref().size()
    }

    fn blocks(&self) -> BufIter<'_> {
        self.as_ref().blocks()
    }
}

struct CompositeOutputBlock {
    blocks: Vec<OutputBlock>,
}

impl OutputBlockImpl for CompositeOutputBlock {
    fn size(&self) -> u64 {
        self.blocks.iter().map(OutputBlock::size).sum()
    }

    fn blocks(&self) -> BufIter<'_> {
        Box::new(self.blocks.iter().flat_map(OutputBlock::blocks))
    }
}

struct BufferOutputBlock<T> {
    buffer: T,
    max_block_size: usize,
}

impl<T> OutputBlockImpl for BufferOutputBlock<T>
where
    T: SplittableBuffer + Send + Sync,
{
    fn size(&self) -> u64 {
        self.buffer.size().try_into().unwrap()
    }

    fn blocks(&self) -> BufIter<'_> {
        let num_blocks = self.buffer.size().div_ceil(self.max_block_size);
        Box::new((0..num_blocks).map(move |i| {
            let start = i * self.max_block_size;
            let end = std::cmp::min(start + self.max_block_size, self.buffer.size());
            ensure_other!(end <= self.buffer.size(), "Block range out of bounds");
            Ok(BlockData::from_buffer(
                self.buffer.clone().sub_buffer_from_range(start, end),
            ))
        }))
    }
}

struct BytesOutputBlock(bytes::Bytes);

impl OutputBlockImpl for BytesOutputBlock {
    fn size(&self) -> u64 {
        self.0.len() as u64
    }

    fn blocks(&self) -> BufIter<'_> {
        let block = BlockData::new(self.0.clone());
        Box::new(std::iter::once(Ok(block)))
    }
}

struct BlockSourceOutputBlock {
    source: BlockSource,
    max_block_size: usize,
}

impl OutputBlockImpl for BlockSourceOutputBlock {
    fn size(&self) -> u64 {
        self.source.size()
    }

    fn blocks(&self) -> BufIter<'_> {
        let num_blocks = self.source.size().div_ceil(self.max_block_size as u64);
        Box::new((0..num_blocks).map(move |i| {
            let start = i * self.max_block_size as u64;
            let end = std::cmp::min(start + self.max_block_size as u64, self.source.size());
            Ok(BlockData::from_buffer(
                self.source
                    .clone()
                    .subblock(start..end)
                    .open()
                    .with_other_err()?,
            ))
        }))
    }
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum WriteError {
    #[doc(hidden)]
    #[error(transparent)]
    Other(#[from] OtherError),
}

#[derive(Clone)]
pub struct OutputBlock(Arc<dyn OutputBlockImpl>);

impl OutputBlock {
    pub fn from_buffer<T>(buffer: T) -> Self
    where
        T: SplittableBuffer + Send + Sync + 'static,
    {
        Self(Arc::new(BufferOutputBlock {
            buffer,
            max_block_size: 4 * 1024 * 1024,
        }))
    }

    #[must_use]
    pub fn from_block_source(source: BlockSource) -> Self {
        Self(Arc::new(BlockSourceOutputBlock {
            source,
            max_block_size: 4 * 1024 * 1024,
        }))
    }

    #[must_use]
    pub fn size(&self) -> u64 {
        self.0.size()
    }

    pub fn blocks(
        &self,
    ) -> impl Iterator<Item = Result<BlockData<'_>, OutputBlockError>> + Unpin + '_ {
        self.0.blocks()
    }

    pub fn write_to<W: Write + Unpin>(&self, mut writer: W) -> Result<(), WriteError> {
        for block in self.blocks() {
            let mut block = block.with_other_err()?;
            while block.has_remaining() {
                let bytes_written = writer.write(block.chunk()).with_other_err()?;
                block.advance(bytes_written);
            }
        }
        Ok(())
    }
}

impl FromIterator<OutputBlock> for OutputBlock {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = OutputBlock>,
    {
        let blocks = iter.into_iter().collect::<Vec<_>>();
        Self(Arc::new(CompositeOutputBlock { blocks }))
    }
}

impl From<bytes::Bytes> for OutputBlock {
    fn from(bytes: bytes::Bytes) -> Self {
        Self(Arc::new(BytesOutputBlock(bytes)))
    }
}
