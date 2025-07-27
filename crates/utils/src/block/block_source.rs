use std::{
    io::{self, Read, Seek},
    ops::RangeBounds,
    path::Path,
    sync::{Arc, Mutex},
};

use crate::buffer::Buffer;

use super::{LazyBlock, MemBlock, ReadError, ReadResult};

trait BlockSourceImpl: Send + Sync {
    fn read_block(&self, start: u64, size: u64) -> ReadResult<MemBlock>;
}

struct ReaderBlockSourceImpl<R>(Mutex<R>);

impl<R> BlockSourceImpl for ReaderBlockSourceImpl<R>
where
    R: io::Read + io::Seek + Send,
{
    fn read_block(&self, start: u64, size: u64) -> ReadResult<MemBlock> {
        let mut reader = self.0.lock().unwrap();
        reader.seek(io::SeekFrom::Start(start))?;
        let mut data = vec![0; size.try_into().map_err(ReadError::from_std_err)?];
        reader.read_exact(&mut data)?;

        Ok(MemBlock::from_vec(data))
    }
}

struct PathBlockSourceImpl<P>(P);

impl<P> BlockSourceImpl for PathBlockSourceImpl<P>
where
    P: AsRef<Path> + Sync + Send,
{
    fn read_block(&self, start: u64, size: u64) -> ReadResult<MemBlock> {
        let mut file = std::fs::File::open(self.0.as_ref())?;
        file.seek(io::SeekFrom::Start(start))?;
        let mut data = vec![0; size.try_into().map_err(ReadError::from_std_err)?];
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
    pub fn from_path<P>(path: P) -> io::Result<Self>
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

    pub fn from_reader<R>(reader: R) -> Self
    where
        R: io::Read + io::Seek + Send + 'static,
    {
        let mut reader = io::BufReader::new(reader);
        let size = reader.seek(io::SeekFrom::End(0)).unwrap();
        Self {
            start: 0,
            size,
            source_impl: Arc::new(ReaderBlockSourceImpl(Mutex::new(reader))),
        }
    }

    /// Returns the size of the block source.
    #[must_use]
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Opens the block source, returning the block of data. Returns an error
    /// if the data cannot be read and/or loaded.
    pub fn open(&self) -> ReadResult<MemBlock> {
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

    /// Returns a lazy block that represents the current block source that can
    /// be opened on demand.
    #[must_use]
    pub fn to_lazy_block(&self) -> LazyBlock {
        LazyBlock::from_block_source(self.clone())
    }
}

impl Buffer for BlockSource {
    type Error = ReadError;
    type Guard<'g>
        = MemBlock
    where
        Self: 'g;

    fn size(&self) -> u64 {
        self.size
    }

    fn sub_buffer_from_range(self, start: u64, end: u64) -> Self {
        self.subblock(start..end)
    }

    fn split_at(self, at: u64) -> (Self, Self) {
        self.split_at(at)
    }

    fn lock_range(&self, start: u64, end: u64) -> Result<Self::Guard<'_>, Self::Error> {
        let block = self.subblock(start..end).open()?;
        Ok(block)
    }

    fn read_value<T: crate::buffer::FromFixedBytes>(self) -> anyhow::Result<(T, Self)> {
        todo!()
    }
}
