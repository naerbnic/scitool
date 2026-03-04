use std::{fmt::Debug, io};

use scidev_errors::AnyDiag;

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
    F: Fn() -> AnyDiag + Clone,
{
    pub(super) fn new(error: F) -> Self {
        Self { error }
    }
}

impl<F> BlockBase for ErrorBlockImpl<F>
where
    F: Fn() -> AnyDiag + Clone,
{
    fn open_mem(&self, _range: BoundedRange<u64>) -> OpenBaseResult<MemBlock> {
        Err((self.error)())
    }

    fn open_reader<'a>(
        &'a self,
        _range: BoundedRange<u64>,
    ) -> OpenBaseResult<Box<dyn io::Read + 'a>> {
        Err((self.error)())
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
