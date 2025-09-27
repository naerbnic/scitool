use std::{
    io::{self},
    ops::RangeBounds,
    path::Path,
    pin::Pin,
    sync::Arc,
};

use tokio::io::{AsyncReadExt as _, AsyncSeekExt as _};

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

type ReadFuture<'a> = Pin<Box<dyn Future<Output = Result<MemBlock, Error>> + Send + 'a>>;

trait BlockSourceImpl: Send + Sync {
    fn read_block(&self, start: u64, size: u64) -> ReadFuture<'_>;
}

struct ReaderBlockSourceImpl<R>(tokio::sync::Mutex<R>);

impl<R> BlockSourceImpl for ReaderBlockSourceImpl<R>
where
    R: tokio::io::AsyncRead + tokio::io::AsyncSeek + Unpin + Send,
{
    fn read_block(&self, start: u64, size: u64) -> ReadFuture<'_> {
        Box::pin(async move {
            let mut reader = self.0.lock().await;
            reader.seek(io::SeekFrom::Start(start)).await?;
            let mut data = vec![0; size.try_into()?];
            reader.read_exact(&mut data).await?;

            Ok(MemBlock::from_vec(data))
        })
    }
}

struct PathBlockSourceImpl<P>(P);

impl<P> BlockSourceImpl for PathBlockSourceImpl<P>
where
    P: AsRef<Path> + Sync + Send,
{
    fn read_block(&self, start: u64, size: u64) -> ReadFuture<'_> {
        Box::pin(async move {
            let mut file = tokio::fs::File::open(self.0.as_ref()).await?;
            file.seek(io::SeekFrom::Start(start)).await?;
            let mut data = vec![0; size.try_into()?];
            file.read_exact(&mut data).await?;

            Ok(MemBlock::from_vec(data))
        })
    }
}

struct VecBlockSourceImpl {
    data: Vec<u8>,
}

impl BlockSourceImpl for VecBlockSourceImpl {
    fn read_block(&self, start: u64, size: u64) -> ReadFuture<'_> {
        Box::pin(async move {
            let start: usize = start.try_into()?;
            let size: usize = size.try_into()?;
            let end = start + size;
            if end > self.data.len() {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    format!(
                        "Tried to read block from {} to {}, but data is only {} bytes long",
                        start,
                        end,
                        self.data.len()
                    ),
                )
                .into());
            }
            Ok(MemBlock::from_vec(self.data[start..end].to_vec()))
        })
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

    pub async fn from_reader<R>(reader: R) -> Result<Self, Error>
    where
        R: tokio::io::AsyncRead + tokio::io::AsyncSeek + Send + Unpin + 'static,
    {
        let mut reader = tokio::io::BufReader::new(reader);
        let size = reader.seek(io::SeekFrom::End(0)).await?;
        Ok(Self {
            start: 0,
            size,
            source_impl: Arc::new(ReaderBlockSourceImpl(tokio::sync::Mutex::new(reader))),
        })
    }

    #[must_use]
    pub fn from_vec(data: Vec<u8>) -> Self {
        let size = data.len() as u64;
        Self {
            start: 0,
            size,
            source_impl: Arc::new(VecBlockSourceImpl { data }),
        }
    }

    /// Returns the size of the block source.
    #[must_use]
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Opens the block source, returning the block of data. Returns an error
    /// if the data cannot be read and/or loaded.
    pub async fn open(&self) -> Result<MemBlock, Error> {
        self.source_impl.read_block(self.start, self.size).await
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
            "Start: {} End: {} Size: {}",
            start,
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

    pub async fn to_buffer(&self) -> Result<impl Buffer, Error> {
        self.open().await
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

pub trait FromBlockSource: mem_reader::Parse {
    #[must_use]
    fn from_block_source(
        source: &BlockSource,
    ) -> impl Future<Output = Result<(Self, BlockSource), FromBlockSourceError>> {
        async move {
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
            let block = source.subblock(..Self::read_size() as u64).open().await?;
            let mut reader = BufferMemReader::from_ref(&block);
            let parse_result = Self::parse(&mut reader);
            let value = parse_result.remove_no_error()?;
            let rest = source.subblock(reader.tell() as u64..);
            Ok((value, rest))
        }
    }

    fn read_size() -> usize;
}
