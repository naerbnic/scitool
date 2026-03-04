use std::{fmt::Debug, io};

use scidev_errors::{AnyDiag, ResultExt as _};

use crate::utils::block::core::{FullStreamBase, OpenBaseResult, RefFactory};

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
    F::Error: Into<AnyDiag>,
    for<'a> F::Output<'a>: io::Read,
{
    type Reader<'a>
        = F::Output<'a>
    where
        Self: 'a;
    fn open_full_reader(&self) -> OpenBaseResult<Self::Reader<'_>> {
        self.0
            .create_new()
            .map_err(Into::into)
            .with_context()
            .msg("Error creating full reader.")
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
