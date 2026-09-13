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

    fn open_mem_async(
        &self,
        range: BoundedRange<u64>,
    ) -> impl Future<Output = OpenBaseResult<MemBlock>> {
        let result = (move || {
            ensure!(range.size() == 0, "Cannot read from empty block");
            Ok(MemBlock::empty())
        })();
        std::future::ready(result)
    }

    fn open_async_reader(
        &self,
        range: BoundedRange<u64>,
    ) -> impl Future<Output = OpenBaseResult<impl AsyncRead + Send + 'static>> {
        let result = (move || {
            ensure!(range.size() == 0, "Cannot read from empty block");
            Ok(Box::new(tokio::io::empty()))
        })();
        std::future::ready(result)
    }
}
