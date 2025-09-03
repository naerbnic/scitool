use std::io;

use crate::utils::mem_reader::{self, BufferMemReader, MemReader};

use super::block::BlockSource;

#[derive(Debug, thiserror::Error)]
pub enum FromBlockSourceError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    MemReader(#[from] mem_reader::Error),
}

pub trait FromBlockSource: Sized {
    fn from_block_source(
        source: &BlockSource,
    ) -> Result<(Self, BlockSource), FromBlockSourceError> {
        let block = source
            .subblock(..Self::read_size() as u64)
            .open()
            .map_err(io::Error::from)?;
        let header = Self::parse(BufferMemReader::new(&block))?;
        let rest = source.subblock(Self::read_size() as u64..);
        Ok((header, rest))
    }

    fn read_size() -> usize;

    fn parse<'a, M: MemReader<'a>>(reader: M) -> mem_reader::Result<Self>;
}
