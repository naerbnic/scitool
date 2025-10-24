use std::{fmt::Debug, io};

use crate::utils::block::core::{FullStreamBase, RefFactory};

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

impl<F> Debug for ReadFactoryImpl<F>
where
    F: RefFactory,
    for<'a> F::Output<'a>: io::Read,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReadFactoryImpl").finish()
    }
}
