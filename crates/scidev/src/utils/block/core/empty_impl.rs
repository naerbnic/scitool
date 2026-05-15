use std::io;

use scidev_errors::ensure;

use crate::utils::{
    block::{
        MemBlock,
        core::{BlockBase, BoxedRead, OpenBaseResult},
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

    fn open_reader(&self, range: BoundedRange<u64>) -> OpenBaseResult<BoxedRead> {
        ensure!(range.size() == 0, "Cannot read from empty block");
        Ok(Box::new(io::empty()))
    }
}
