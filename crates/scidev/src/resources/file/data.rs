mod contents;
mod raw_header;

use std::io;

use crate::{
    resources::ConversionError,
    utils::{
        block::{BlockSource, FromBlockSource, FromBlockSourceError},
        errors::AnyInvalidDataError,
    },
};

use super::map::ResourceLocation;
use contents::Contents;
use raw_header::RawEntryHeader;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    MemReader(#[from] AnyInvalidDataError),
    #[error(transparent)]
    Conversion(#[from] ConversionError),
}

impl From<FromBlockSourceError> for Error {
    fn from(err: FromBlockSourceError) -> Self {
        match err {
            FromBlockSourceError::Io(io_err) => Self::Io(io_err),
            FromBlockSourceError::MemReader(mem_err) => Self::MemReader(mem_err),
            FromBlockSourceError::Conversion(err) => Self::Conversion(ConversionError::new(err)),
        }
    }
}

pub(crate) struct DataFile {
    data: BlockSource,
}

impl DataFile {
    pub(crate) fn new(data: BlockSource) -> DataFile {
        DataFile { data }
    }

    pub(crate) fn read_contents(&self, location: ResourceLocation) -> Result<Contents, Error> {
        let (header, rest) = RawEntryHeader::from_block_source(
            &self.data.subblock(u64::from(location.file_offset)..),
        )?;
        let resource_block = rest.subblock(..u64::from(header.packed_size()));
        Ok(Contents::from_parts(header, resource_block)?)
    }
}
