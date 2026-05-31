use std::{fmt::Debug, io};

use scidev_errors::AnyDiag;
use tokio::io::AsyncRead;

use crate::utils::{
    block::{
        MemBlock,
        core::{BlockBase, OpenBaseResult},
    },
    range::BoundedRange,
};

pub(super) struct ErrorBlockImpl<F> {
    error: F,
}

impl<F> ErrorBlockImpl<F>
where
    F: Fn() -> AnyDiag + Clone + Sync,
{
    pub(super) fn new(error: F) -> Self {
        Self { error }
    }
}

impl<F> BlockBase for ErrorBlockImpl<F>
where
    F: Fn() -> AnyDiag + Clone + Sync,
{
    fn open_mem(&self, _range: BoundedRange<u64>) -> OpenBaseResult<MemBlock> {
        Err((self.error)())
    }

    fn open_reader(
        &self,
        _range: BoundedRange<u64>,
    ) -> OpenBaseResult<impl io::Read + Send + 'static> {
        Err::<&'static [u8], _>((self.error)())
    }

    async fn open_mem_async(&self, _range: BoundedRange<u64>) -> OpenBaseResult<MemBlock> {
        Err((self.error)())
    }

    async fn open_async_reader(
        &self,
        _range: BoundedRange<u64>,
    ) -> OpenBaseResult<impl AsyncRead + Send + 'static> {
        Err::<&'static [u8], _>((self.error)())
    }
}

impl<F> Debug for ErrorBlockImpl<F>
where
    F: Fn() -> AnyDiag + Clone,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ErrorBlockImpl").finish()
    }
}
