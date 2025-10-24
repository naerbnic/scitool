use std::io::Write;

use bytes::BufMut;

use crate::utils::{
    block::Block,
    buffer::{BufferCursor, SplittableBuffer},
    errors::{OtherError, prelude::*},
};

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum WriteError {
    #[doc(hidden)]
    #[error(transparent)]
    Other(#[from] OtherError),
}

#[derive(Clone)]
pub struct OutputBlock(Block);

impl OutputBlock {
    pub fn from_buffer<T>(buffer: T) -> Self
    where
        T: SplittableBuffer + Send + Sync + 'static,
    {
        let mut data = Vec::new();
        data.put(BufferCursor::new(buffer));
        Self(Block::from_vec(data))
    }

    #[must_use]
    pub fn from_block_source(source: Block) -> Self {
        Self(source)
    }

    #[must_use]
    pub fn size(&self) -> u64 {
        self.0.len()
    }

    pub fn write_to<W: Write + Unpin>(&self, mut writer: W) -> Result<(), WriteError> {
        let mut reader = self.0.open_reader(..).with_other_err()?;
        std::io::copy(&mut reader, &mut writer).with_other_err()?;
        Ok(())
    }
}

impl FromIterator<OutputBlock> for OutputBlock {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = OutputBlock>,
    {
        Self(Block::concat(iter.into_iter().map(|b| b.0)))
    }
}

impl From<bytes::Bytes> for OutputBlock {
    fn from(bytes: bytes::Bytes) -> Self {
        let mut data = Vec::new();
        data.put(bytes);
        Self(Block::from_vec(data))
    }
}
