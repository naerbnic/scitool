use std::io;

use scidev_errors::ensure;
use tokio::io::AsyncRead;

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

    fn open_reader(
        &self,
        range: BoundedRange<u64>,
    ) -> OpenBaseResult<impl io::Read + Send + 'static> {
        ensure!(range.size() == 0, "Cannot read from empty block");
        Ok(Box::new(io::empty()))
    }

    async fn open_mem_async(&self, range: BoundedRange<u64>) -> OpenBaseResult<MemBlock> {
        ensure!(range.size() == 0, "Cannot read from empty block");
        Ok(MemBlock::empty())
    }

    async fn open_async_reader(
        &self,
        range: BoundedRange<u64>,
    ) -> OpenBaseResult<impl AsyncRead + Send + 'static> {
        ensure!(range.size() == 0, "Cannot read from empty block");
        Ok(Box::new(tokio::io::empty()))
    }
}
