use std::sync::Arc;

use crate::buffer::Buffer;

pub struct BlockData<'a>(Box<dyn bytes::Buf + 'a>);

impl<'a> BlockData<'a> {
    pub fn new<T>(buf: T) -> Self
    where
        T: bytes::Buf + 'a,
    {
        Self(Box::new(buf))
    }
}

impl bytes::Buf for BlockData<'_> {
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

pub struct BufferOutputBlock<T>(T);

impl<T> OutputBlockImpl for BufferOutputBlock<T>
where
    T: Buffer + Send + Sync,
{
    fn size(&self) -> u64 {
        self.0.size()
    }

    fn blocks(&self) -> BufIter<'_> {
        let guard = self.0.lock().map_err(|e| e.into()).map(BlockData::new);
        Box::new(std::iter::once(guard))
    }
}

pub struct OutputBlock(Arc<dyn OutputBlockImpl>);

impl OutputBlock {
    pub fn from_buffer<T>(buffer: T) -> Self
    where
        T: Buffer + Send + Sync + 'static,
    {
        Self(Arc::new(BufferOutputBlock(buffer)))
    }

    pub fn size(&self) -> u64 {
        self.0.size()
    }

    pub fn blocks(&self) -> impl Iterator<Item = anyhow::Result<BlockData<'_>>> + '_ {
        self.0.blocks()
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
