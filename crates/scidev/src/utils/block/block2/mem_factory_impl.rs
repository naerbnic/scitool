use std::{fmt::Debug, io};

use crate::utils::block::{
    MemBlock,
    block2::{MemBlockBase, RefFactory},
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
    for<'a> F::Output<'a>: Into<MemBlock>,
{
    fn load_mem_block(&self) -> io::Result<MemBlock> {
        Ok(self.0.create_new()?.into())
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
