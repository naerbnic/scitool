use std::io;

use crate::utils::block::{MemBlock, block2::MemBlockBase};

#[derive(Debug)]
pub(super) struct ContainedMemBlock(MemBlock);

impl ContainedMemBlock {
    pub(super) fn new(mem_block: MemBlock) -> Self {
        Self(mem_block)
    }
}

impl MemBlockBase for ContainedMemBlock {
    fn load_mem_block(&self) -> io::Result<MemBlock> {
        Ok(self.0.clone())
    }
}
