use std::{fmt::Debug, io};

use scidev_errors::{AnyDiag, ResultExt as _};

use crate::utils::block::core::{FullStreamBase, OpenBaseResult, RefFactory};

pub(super) struct ReadFactoryImpl<F>(F);

impl<F> ReadFactoryImpl<F>
where
    F: RefFactory,
    F::Output: io::Read + Send + 'static,
{
    pub(super) fn new(factory: F) -> Self {
        Self(factory)
    }
}

impl<F> FullStreamBase for ReadFactoryImpl<F>
where
    F: RefFactory,
    F::Error: Into<AnyDiag>,
    F::Output: io::Read + Send + 'static,
{
    type Reader = F::Output;
    fn open_full_reader(&self) -> OpenBaseResult<Self::Reader> {
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
    F::Output: io::Read + Send + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReadFactoryImpl").finish()
    }
}
