use std::io;

use scidev_errors::ensure;

use crate::utils::{
    block::{
        MemBlock,
        core::{BlockBase, OpenBaseResult},
    },
    range::BoundedRange,
};

#[derive(Debug)]
pub(super) struct EmptyBlockImpl;

impl BlockBase for EmptyBlockImpl {
    fn open_mem(&self, range: BoundedRange<u64>) -> OpenBaseResult<MemBlock> {
        ensure!(range.size() == 0, "Cannot read from empty block");
        Ok(MemBlock::empty())
    }

    fn open_reader<'a>(
        &'a self,
        range: BoundedRange<u64>,
    ) -> OpenBaseResult<Box<dyn io::Read + 'a>> {
        ensure!(range.size() == 0, "Cannot read from empty block");
        Ok(Box::new(io::empty()))
    }
}
