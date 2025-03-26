use std::sync::Arc;

use bytes::Buf;

use crate::buffer::Buffer;
use futures::io::AsyncWriteExt;

pub struct BlockData<'a>(Box<dyn bytes::Buf + 'a>);

impl<'a> BlockData<'a> {
    pub fn new<T>(buf: T) -> Self
    where
        T: bytes::Buf + 'a,
    {
        Self(Box::new(buf))
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
        self.0.advance(cnt)
    }
}

type BufIter<'a> = Box<dyn Iterator<Item = anyhow::Result<BlockData<'a>>> + 'a>;

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

pub struct CompositeOutputBlock {
    blocks: Vec<OutputBlock>,
}

impl OutputBlockImpl for CompositeOutputBlock {
    fn size(&self) -> u64 {
        self.blocks.iter().map(|b| b.size()).sum()
    }

    fn blocks(&self) -> BufIter<'_> {
        Box::new(self.blocks.iter().flat_map(|b| b.blocks()))
    }
}

struct BufferOutputBlock<T> {
    buffer: T,
    max_block_size: usize,
}

impl<T> OutputBlockImpl for BufferOutputBlock<T>
where
    T: Buffer + Send + Sync,
{
    fn size(&self) -> u64 {
        self.buffer.size()
    }

    fn blocks(&self) -> BufIter<'_> {
        let num_blocks = self.size().div_ceil(self.max_block_size as u64);
        Box::new((0..num_blocks).map(move |i| {
            let start = i * self.max_block_size as u64;
            let end = std::cmp::min(start + self.max_block_size as u64, self.size());
            Ok(BlockData::new(self.buffer.lock_range(start, end)?))
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

pub struct OutputBlock(Arc<dyn OutputBlockImpl>);

impl OutputBlock {
    pub fn from_buffer<T>(buffer: T) -> Self
    where
        T: Buffer + Send + Sync + 'static,
    {
        Self(Arc::new(BufferOutputBlock {
            buffer,
            max_block_size: 4 * 1024 * 1024,
        }))
    }

    pub fn size(&self) -> u64 {
        self.0.size()
    }

    pub fn blocks(&self) -> impl Iterator<Item = anyhow::Result<BlockData<'_>>> + '_ {
        self.0.blocks()
    }

    pub fn write_to<R: std::io::Write>(&self, writer: &mut R) -> anyhow::Result<()> {
        for block in self.blocks() {
            let mut block = block?;
            while block.has_remaining() {
                let bytes_written = writer.write(block.chunk())?;
                block.advance(bytes_written);
            }
        }
        Ok(())
    }

    pub async fn write_to_async<W: futures::io::AsyncWrite + Unpin>(
        &self,
        writer: &mut W,
    ) -> anyhow::Result<()> {
        for block in self.blocks() {
            let mut block = block?;
            while block.has_remaining() {
                let bytes_written = writer.write(block.chunk()).await?;
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
