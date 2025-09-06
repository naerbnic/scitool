use std::{
    io::{self, Read, Seek},
    ops::RangeBounds,
    path::Path,
    sync::{Arc, Mutex},
};

use crate::utils::{
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

trait BlockSourceImpl: Send + Sync {
    fn read_block(&self, start: u64, size: u64) -> Result<MemBlock, Error>;
}

struct ReaderBlockSourceImpl<R>(Mutex<R>);

impl<R> BlockSourceImpl for ReaderBlockSourceImpl<R>
where
    R: io::Read + io::Seek + Send,
{
    fn read_block(&self, start: u64, size: u64) -> Result<MemBlock, Error> {
        let mut reader = self.0.lock().unwrap();
        reader.seek(io::SeekFrom::Start(start))?;
        let mut data = vec![0; size.try_into()?];
        reader.read_exact(&mut data)?;

        Ok(MemBlock::from_vec(data))
    }
}

struct PathBlockSourceImpl<P>(P);

impl<P> BlockSourceImpl for PathBlockSourceImpl<P>
where
    P: AsRef<Path> + Sync + Send,
{
    fn read_block(&self, start: u64, size: u64) -> Result<MemBlock, Error> {
        let mut file = std::fs::File::open(self.0.as_ref())?;
        file.seek(io::SeekFrom::Start(start))?;
        let mut data = vec![0; size.try_into()?];
        file.read_exact(&mut data)?;

        Ok(MemBlock::from_vec(data))
    }
}

/// A source of blocks. These can be loaded lazily, and still can be split
/// into sub-block-sources.
#[derive(Clone)]
pub struct BlockSource {
    start: u64,
    size: u64,
    source_impl: Arc<dyn BlockSourceImpl>,
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
            start: 0,
            size,
            source_impl: Arc::new(PathBlockSourceImpl(path)),
        })
    }

    pub fn from_reader<R>(reader: R) -> Result<Self, Error>
    where
        R: io::Read + io::Seek + Send + 'static,
    {
        let mut reader = io::BufReader::new(reader);
        let size = reader.seek(io::SeekFrom::End(0))?;
        Ok(Self {
            start: 0,
            size,
            source_impl: Arc::new(ReaderBlockSourceImpl(Mutex::new(reader))),
        })
    }

    /// Returns the size of the block source.
    #[must_use]
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Opens the block source, returning the block of data. Returns an error
    /// if the data cannot be read and/or loaded.
    pub fn open(&self) -> Result<MemBlock, Error> {
        self.source_impl.read_block(self.start, self.size)
    }

    /// Returns a sub-block source that represents a subrange of the current
    /// block source.
    #[must_use]
    pub fn subblock<R>(&self, range: R) -> Self
    where
        R: RangeBounds<u64>,
    {
        let start = match range.start_bound() {
            std::ops::Bound::Included(&start) => start,
            std::ops::Bound::Excluded(&start) => start + 1,
            std::ops::Bound::Unbounded => 0,
        };

        let end = match range.end_bound() {
            std::ops::Bound::Included(&end) => end + 1,
            std::ops::Bound::Excluded(&end) => end,
            std::ops::Bound::Unbounded => self.size,
        };

        // Actual start/end are offsets from self.start
        let start = self.start + start;
        let end = self.start + end;

        assert!(start <= end);
        assert!(
            end <= self.start + self.size,
            "End: {} Size: {}",
            end,
            self.start + self.size
        );

        Self {
            start,
            size: end - start,
            source_impl: self.source_impl.clone(),
        }
    }

    #[must_use]
    pub fn split_at(self, at: u64) -> (Self, Self) {
        assert!(
            at <= self.size,
            "Tried to split a block of size {} at offset {}",
            self.size,
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
        LazyBlock::from_block_source(self.clone())
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

pub trait FromBlockSource: Sized {
    fn from_block_source(
        source: &BlockSource,
    ) -> Result<(Self, BlockSource), FromBlockSourceError> {
        let block = source.subblock(..Self::read_size() as u64).open()?;
        let parse_result = Self::parse(BufferMemReader::new(block));
        let header = parse_result.remove_no_error()?;
        let rest = source.subblock(Self::read_size() as u64..);
        Ok((header, rest))
    }

    fn read_size() -> usize;

    fn parse<M: MemReader>(reader: M) -> mem_reader::Result<Self, M::Error>;
}
