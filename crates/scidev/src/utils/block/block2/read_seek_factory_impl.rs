use std::{
    fmt::Debug,
    io::{self, Read as _, Seek as _},
};

use crate::utils::{
    block::block2::{RangeStreamBase, RefFactory},
    range::BoundedRange,
};

pub(super) struct ReadSeekFactorySource<F>(F);

impl<F> ReadSeekFactorySource<F>
where
    F: RefFactory,
    for<'a> F::Output<'a>: io::Read + io::Seek,
{
    pub(super) fn new(factory: F) -> Self {
        Self(factory)
    }
}

impl<F> RangeStreamBase for ReadSeekFactorySource<F>
where
    F: RefFactory,
    for<'a> F::Output<'a>: io::Read + io::Seek,
{
    type Reader<'a>
        = io::Take<F::Output<'a>>
    where
        Self: 'a;
    fn open_range_reader(&self, range: BoundedRange<u64>) -> io::Result<Self::Reader<'_>> {
        let mut reader = self.0.create_new()?;
        reader.seek(io::SeekFrom::Start(range.start()))?;
        Ok(reader.take(range.size()))
    }
}

impl<F> Debug for ReadSeekFactorySource<F>
where
    F: RefFactory,
    for<'a> F::Output<'a>: io::Read + io::Seek,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReadSeekFactorySource").finish()
    }
}
