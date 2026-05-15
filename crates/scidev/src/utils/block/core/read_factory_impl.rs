use std::{fmt::Debug, io, marker::PhantomData};

use scidev_errors::{AnyDiag, ResultExt as _};

use crate::utils::block::core::{FullStreamBase, OpenBaseResult};

pub(super) struct ReadFactoryImpl<F, Out, E> {
    func: F,
    _phantom: PhantomData<fn() -> Result<Out, E>>,
}

impl<F, Out, E> ReadFactoryImpl<F, Out, E>
where
    F: Fn() -> Result<Out, E>,
    Out: io::Read + Send + 'static,
    E: Into<AnyDiag>,
{
    pub(super) fn new(factory: F) -> Self {
        Self {
            func: factory,
            _phantom: PhantomData,
        }
    }
}

impl<F, Out, E> FullStreamBase for ReadFactoryImpl<F, Out, E>
where
    F: Fn() -> Result<Out, E>,
    Out: io::Read + Send + 'static,
    E: Into<AnyDiag>,
{
    type Reader = Out;
    fn open_full_reader(&self) -> OpenBaseResult<Self::Reader> {
        (self.func)()
            .map_err(Into::into)
            .with_context()
            .msg("Error creating full reader.")
    }
}

impl<F, Out, E> Debug for ReadFactoryImpl<F, Out, E>
where
    F: Fn() -> Result<Out, E>,
    Out: io::Read + Send + 'static,
    E: Into<AnyDiag>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReadFactoryImpl").finish()
    }
}
