use std::io;

use crate::utils::block::block2::{FullStreamBase, RefFactory};

pub(super) struct ReadFactoryImpl<F>(F);

impl<F> ReadFactoryImpl<F>
where
    F: RefFactory,
    for<'a> F::Output<'a>: io::Read,
{
    pub(super) fn new(factory: F) -> Self {
        Self(factory)
    }
}

impl<F> FullStreamBase for ReadFactoryImpl<F>
where
    F: RefFactory,
    for<'a> F::Output<'a>: io::Read,
{
    type Reader<'a>
        = F::Output<'a>
    where
        Self: 'a;
    fn open_full_reader(&self) -> io::Result<Self::Reader<'_>> {
        self.0.create_new()
    }
}
