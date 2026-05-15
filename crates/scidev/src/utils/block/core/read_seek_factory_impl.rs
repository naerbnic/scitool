use std::{
    fmt::Debug,
    io::{self},
    marker::PhantomData,
};

use scidev_errors::{AnyDiag, diag, prelude::*};

use crate::utils::{
    block::core::{OpenBaseResult, RangeStreamBase},
    range::BoundedRange,
};

pub(super) struct ReadSeekFactorySource<F, Out, E> {
    func: F,
    _phantom: PhantomData<fn() -> Result<Out, E>>,
}

impl<F, Out, E> ReadSeekFactorySource<F, Out, E>
where
    F: Fn() -> Result<Out, E> + Send + Sync + 'static,
    E: Into<AnyDiag>,
    Out: io::Read + io::Seek + Send,
{
    pub(super) fn new(factory: F) -> Self {
        Self {
            func: factory,
            _phantom: PhantomData,
        }
    }
}

impl<F, Out, E> RangeStreamBase for ReadSeekFactorySource<F, Out, E>
where
    F: Fn() -> Result<Out, E> + Send + Sync + 'static,
    E: Into<AnyDiag>,
    Out: io::Read + io::Seek + Send + 'static,
{
    type Reader = io::Take<Out>;
    fn open_range_reader(&self, range: BoundedRange<u64>) -> OpenBaseResult<Self::Reader> {
        let mut reader = (self.func)()
            .map_err(Into::into)
            .with_context()
            .msg("Failed to create base reader")?;
        reader
            .seek(io::SeekFrom::Start(range.start()))
            .raise_err_with(diag!(|| "Failed to seek to start of range {range:?}"))?;
        Ok(reader.take(range.size()))
    }
}

impl<F, Out, E> Debug for ReadSeekFactorySource<F, Out, E>
where
    F: Fn() -> Result<Out, E> + Send + Sync + 'static,
    E: Into<AnyDiag>,
    Out: io::Read + io::Seek + Send,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReadSeekFactorySource").finish()
    }
}
