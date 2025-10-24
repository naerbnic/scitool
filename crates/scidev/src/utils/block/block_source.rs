use std::{io, ops::RangeBounds, path::Path};

use crate::utils::{
    block::block2::{Block, Builder},
    buffer::Buffer,
    errors::{AnyInvalidDataError, NoError},
    mem_reader::{self, BufferMemReader, MemReader, NoErrorResultExt as _},
};

use super::{LazyBlock, MemBlock};

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Io(#[from] io::Error),

    #[error("Conversion error: {0}")]
    Conversion(#[from] std::num::TryFromIntError),
}

/// A source of blocks. These can be loaded lazily, and still can be split
/// into sub-block-sources.
#[derive(Clone)]
pub struct BlockSource {
    block: Block,
}

impl BlockSource {
    /// Creates a block source that represents the contents of a path at the
    /// given path. Returns an error if the file cannot be opened.
    pub fn from_path<P>(path: P) -> Result<Self, Error>
    where
        P: AsRef<Path> + Send + Sync + 'static,
    {
        let size = std::fs::metadata(path.as_ref())?.len();
        Ok(Self {
            block: Builder::new()
                .with_size(size)
                .build_from_read_seek_factory(move || std::fs::File::open(path.as_ref()))?,
        })
    }

    pub fn from_reader<R>(reader: R) -> Result<Self, Error>
    where
        R: io::Read + io::Seek + Send + 'static,
    {
        Ok(Self {
            block: Builder::new().build_from_read_seek(reader)?,
        })
    }

    #[must_use]
    pub fn from_vec(data: Vec<u8>) -> Self {
        Self {
            block: Block::from_mem_block(MemBlock::from_vec(data)),
        }
    }

    pub fn inner(&self) -> &Block {
        &self.block
    }

    /// Returns the size of the block source.
    #[must_use]
    pub fn size(&self) -> u64 {
        self.block.len()
    }

    /// Opens the block source, returning the block of data. Returns an error
    /// if the data cannot be read and/or loaded.
    pub fn open(&self) -> Result<MemBlock, Error> {
        Ok(self.block.open_mem(..)?)
    }

    /// Returns a sub-block source that represents a subrange of the current
    /// block source.
    #[must_use]
    pub fn subblock<R>(&self, range: R) -> Self
    where
        R: RangeBounds<u64>,
    {
        Self {
            block: self.block.subblock(range),
        }
    }

    #[must_use]
    pub fn split_at(self, at: u64) -> (Self, Self) {
        assert!(
            at <= self.size(),
            "Tried to split a block of size {} at offset {}",
            self.size(),
            at
        );
        (self.clone().subblock(..at), self.subblock(at..))
    }

    pub fn to_buffer(&self) -> Result<impl Buffer, Error> {
        self.open()
    }

    /// Returns a lazy block that represents the current block source that can
    /// be opened on demand.
    #[must_use]
    pub fn to_lazy_block(&self) -> LazyBlock {
        LazyBlock::from_block_source(self.block.clone())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FromBlockSourceError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    MemReader(#[from] AnyInvalidDataError),
    #[error(transparent)]
    Conversion(#[from] std::num::TryFromIntError),
}

impl From<mem_reader::Error<NoError>> for FromBlockSourceError {
    fn from(err: mem_reader::Error<NoError>) -> Self {
        match err {
            mem_reader::Error::InvalidData(invalid_data_err) => Self::MemReader(invalid_data_err),
            mem_reader::Error::BaseError(err) => err.absurd(),
        }
    }
}

impl From<Error> for FromBlockSourceError {
    fn from(err: Error) -> Self {
        match err {
            Error::Io(err) => Self::Io(err),
            Error::Conversion(err) => Self::Conversion(err),
        }
    }
}

pub trait FromBlockSource: mem_reader::Parse {
    fn from_block_source(
        source: &BlockSource,
    ) -> Result<(Self, BlockSource), FromBlockSourceError> {
        if Self::read_size() as u64 > source.size() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!(
                    "Tried to read {} bytes from block source of size {}",
                    Self::read_size(),
                    source.size()
                ),
            )
            .into());
        }
        let block = source.subblock(..Self::read_size() as u64).open()?;
        let mut reader = BufferMemReader::from_ref(&block);
        let parse_result = Self::parse(&mut reader);
        let value = parse_result.remove_no_error()?;
        let rest = source.subblock(reader.tell() as u64..);
        Ok((value, rest))
    }

    fn read_size() -> usize;
}
