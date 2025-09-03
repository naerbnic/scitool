use crate::utils::buffer::BufferExt;

use super::MemBlock;

/// A [`DataReader`] that reads from a block.
#[derive(Debug, Clone)]
pub struct BlockReader {
    curr_pos: usize,
    block: MemBlock,
}

impl BlockReader {
    /// Creates a new reader from the block.
    #[must_use]
    pub fn new(block: MemBlock) -> Self {
        Self { curr_pos: 0, block }
    }

    /// Returns the portion of the block that has not yet been read.
    #[must_use]
    pub fn into_rest(self) -> MemBlock {
        self.block.sub_buffer(self.curr_pos..).unwrap()
    }
}
