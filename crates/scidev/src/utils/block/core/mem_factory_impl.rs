use std::{fmt::Debug, marker::PhantomData};

use scidev_errors::{AnyDiag, ResultExt};

use crate::utils::block::{
    MemBlock,
    core::{MemBlockBase, OpenBaseResult},
};

pub(super) struct MemFactoryImpl<F, Out, E> {
    func: F,
    _phantom: PhantomData<fn() -> Result<Out, E>>,
}

impl<F, Out, E> MemFactoryImpl<F, Out, E>
where
    F: Fn() -> Result<Out, E>,
    Out: Into<MemBlock>,
{
    pub(super) fn new(factory: F) -> Self {
        Self {
            func: factory,
            _phantom: PhantomData,
        }
    }
}

impl<F, Out, E> MemBlockBase for MemFactoryImpl<F, Out, E>
where
    F: Fn() -> Result<Out, E>,
    E: Into<AnyDiag>,
    Out: Into<MemBlock>,
{
    fn load_mem_block(&self) -> OpenBaseResult<MemBlock> {
        Ok((self.func)()
            .map_err(Into::into)
            .with_context()
            .msg("Error creating MemBlock")?
            .into())
    }
}

impl<F, Out, E> Debug for MemFactoryImpl<F, Out, E>
where
    F: Fn() -> Result<Out, E>,
    E: Into<AnyDiag>,
    Out: Into<MemBlock>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemFactoryImpl").finish()
    }
}
