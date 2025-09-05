use std::io;

use crate::utils::{
    errors::AnyInvalidDataError,
    mem_reader::{self, BufferMemReader, MemReader, NoErrorResultExt as _},
};

use super::block::BlockSource;

#[derive(Debug, thiserror::Error)]
pub enum FromBlockSourceError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    MemReader(#[from] AnyInvalidDataError),
}

pub trait FromBlockSource: Sized {
    fn from_block_source(
        source: &BlockSource,
    ) -> Result<(Self, BlockSource), FromBlockSourceError> {
        let block = source
            .subblock(..Self::read_size() as u64)
            .open()
            .map_err(io::Error::from)?;
        let parse_result = Self::parse(BufferMemReader::new(&block));
        let header = parse_result.remove_no_error()?;
        let rest = source.subblock(Self::read_size() as u64..);
        Ok((header, rest))
    }

    fn read_size() -> usize;

    fn parse<M: MemReader>(reader: M) -> mem_reader::Result<Self, M::Error>;
}
