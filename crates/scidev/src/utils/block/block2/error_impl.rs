use std::io;

use crate::utils::{
    block::{MemBlock, block2::BlockBase},
    range::BoundedRange,
};

pub(super) struct ErrorBlockImpl<E> {
    error: E,
}

impl<E> ErrorBlockImpl<E>
where
    E: Into<io::Error> + Clone,
{
    pub(super) fn new(error: E) -> Self {
        Self { error }
    }
}

impl<E> BlockBase for ErrorBlockImpl<E>
where
    E: Into<io::Error> + Clone,
{
    fn open_mem(&self, _range: BoundedRange<u64>) -> io::Result<MemBlock> {
        Err(self.error.clone().into())
    }

    fn open_reader<'a>(&'a self, _range: BoundedRange<u64>) -> io::Result<Box<dyn io::Read + 'a>> {
        Err(self.error.clone().into())
    }
}
