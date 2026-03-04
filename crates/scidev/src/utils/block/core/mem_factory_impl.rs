use std::fmt::Debug;

use scidev_errors::{AnyDiag, ResultExt};

use crate::utils::block::{
    MemBlock,
    core::{MemBlockBase, OpenBaseResult, RefFactory},
};

pub(super) struct MemFactoryImpl<F>(F);

impl<F> MemFactoryImpl<F>
where
    F: RefFactory,
    for<'a> F::Output<'a>: Into<MemBlock>,
{
    pub(super) fn new(factory: F) -> Self {
        Self(factory)
    }
}

impl<F> MemBlockBase for MemFactoryImpl<F>
where
    F: RefFactory,
    F::Error: Into<AnyDiag>,
    for<'a> F::Output<'a>: Into<MemBlock>,
{
    fn load_mem_block(&self) -> OpenBaseResult<MemBlock> {
        Ok(self
            .0
            .create_new()
            .map_err(Into::into)
            .with_context()
            .msg("Error creating MemBlock")?
            .into())
    }
}

impl<F> Debug for MemFactoryImpl<F>
where
    F: RefFactory,
    for<'a> F::Output<'a>: Into<MemBlock>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemFactoryImpl").finish()
    }
}
