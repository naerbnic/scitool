//! Types that are used to work with ranges of bytes data.

use std::{
    any::Any,
    io::{self, Seek},
    ops::RangeBounds,
    path::Path,
    sync::{Arc, Mutex},
};

use crate::buffer::{Buffer, FromFixedBytes};

use super::data_reader::DataReader;

fn try_cast_to<Target, T>(value: T) -> Result<Target, T>
where
    T: 'static,
    Target: 'static,
{
    match (Box::new(value) as Box<dyn Any>).downcast::<Target>() {
        Ok(target) => Ok(*target),
        Err(value) => Err(*value.downcast::<T>().unwrap()),
    }
}

/// An error that occurs while loading a block value.
#[derive(thiserror::Error)]
#[error(transparent)]
pub struct ReadError(io::Error);

impl std::fmt::Debug for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.0, f)
    }
}

impl ReadError {
    /// Create a new error from an [`io::Error`].
    pub fn new(err: io::Error) -> Self {
        Self(err)
    }

    /// Create a new error from an implementation of [`std::error::Error`].
    pub fn from_std_err<E>(err: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        // This should get optimized away.
        match try_cast_to(err) {
            Ok(io_err) => Self(io_err),
            Err(err) => Self(io::Error::other(err)),
        }
    }
}

impl From<ReadError> for io::Error {
    fn from(err: ReadError) -> Self {
        err.0
    }
}

impl From<io::Error> for ReadError {
    fn from(err: io::Error) -> Self {
        Self(err)
    }
}

/// The result of a read operation.
pub type ReadResult<T> = std::result::Result<T, ReadError>;

/// An in-memory block of data that is cheap to clone, and create subranges of.
#[derive(Clone)]
pub struct Block {
    start: usize,
    size: usize,
    data: Arc<Vec<u8>>,
}

impl Block {
    /// Create the block from a vector of bytes.
    pub fn from_vec(data: Vec<u8>) -> Self {
        let size = data.len();
        Self {
            start: 0,
            size,
            data: Arc::new(data),
        }
    }

    /// Read the entirety of a reader into a block.
    pub fn from_reader<R>(mut reader: R) -> io::Result<Self>
    where
        R: io::Read + io::Seek,
    {
        let size = reader.seek(io::SeekFrom::End(0))?;
        let mut data = vec![0; size.try_into().map_err(ReadError::from_std_err)?];
        reader.seek(io::SeekFrom::Start(0))?;
        reader.read_exact(&mut data)?;
        Ok(Self::from_vec(data))
    }

    /// Returns the size of the block.
    pub fn size(&self) -> usize {
        self.size
    }

    /// Reads a slice of the block into a mutable slice. Returns a read error
    /// if the slice is out of bounds.
    pub fn read_at(&self, offset: usize, buf: &mut [u8]) -> ReadResult<()> {
        if offset + buf.len() > self.size {
            return Err(ReadError::new(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Attempted to read past the end of the block",
            )));
        }

        buf.copy_from_slice(&self.data[self.start + offset..][..buf.len()]);
        Ok(())
    }

    /// Read the entirety of the buffer into a vector.
    pub fn read_all(&self) -> ReadResult<Vec<u8>> {
        let mut buf = vec![0; self.size];
        self.read_at(0, &mut buf)?;
        Ok(buf)
    }

    /// Returns the offset of the contained block within the current block.
    ///
    /// Panics if the argument originated from another block, and is not fully
    /// contained within the current block
    pub fn offset_in(&self, contained_block: &Block) -> usize {
        assert!(Arc::ptr_eq(&self.data, &contained_block.data));
        assert!(self.start <= contained_block.start);
        assert!(contained_block.start + contained_block.size <= self.start + self.size);
        contained_block.start - self.start
    }
}

impl std::ops::Deref for Block {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.data[self.start..][..self.size]
    }
}

impl AsRef<[u8]> for Block {
    fn as_ref(&self) -> &[u8] {
        &self.data[self.start..][..self.size]
    }
}

impl Buffer<'static> for Block {
    type Idx = usize;
    fn size(&self) -> usize {
        self.size
    }

    fn sub_buffer<R: RangeBounds<usize>>(self, range: R) -> Self {
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
            data: self.data,
        }
    }

    fn split_at(self, at: usize) -> (Self, Self) {
        assert!(
            at <= self.size,
            "Tried to split a block of size {} at offset {}",
            self.size,
            at
        );
        (self.clone().sub_buffer(..at), self.sub_buffer(at..))
    }

    fn read_value<T: FromFixedBytes>(self) -> anyhow::Result<(T, Self)> {
        let value_bytes: &[u8] = &self[..T::SIZE];
        let value = T::parse(value_bytes)?;
        let remaining = self.sub_buffer(T::SIZE..);
        Ok((value, remaining))
    }
}

impl std::fmt::Debug for Block {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_tuple("Block")
            .field(&&self.data[self.start..][..self.size])
            .finish()
    }
}

trait BlockSourceImpl: Send + Sync {
    fn read_block(&self, start: u64, size: u64) -> ReadResult<Block>;
}

struct ReaderBlockSourceImpl<R>(Mutex<R>);

impl<R> BlockSourceImpl for ReaderBlockSourceImpl<R>
where
    R: io::Read + io::Seek + Send,
{
    fn read_block(&self, start: u64, size: u64) -> ReadResult<Block> {
        let mut reader = self.0.lock().unwrap();
        reader.seek(io::SeekFrom::Start(start))?;
        let mut data = vec![0; size.try_into().map_err(ReadError::from_std_err)?];
        reader.read_exact(&mut data)?;

        Ok(Block::from_vec(data))
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
    pub fn from_path(path: &Path) -> io::Result<Self> {
        let mut file = std::fs::File::open(path)?;
        let size = file.seek(io::SeekFrom::End(0))?;
        Ok(Self {
            start: 0,
            size,
            source_impl: Arc::new(ReaderBlockSourceImpl(Mutex::new(io::BufReader::new(file)))),
        })
    }

    /// Returns the size of the block source.
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Opens the block source, returning the block of data. Returns an error
    /// if the data cannot be read and/or loaded.
    pub fn open(&self) -> ReadResult<Block> {
        self.source_impl.read_block(self.start, self.size)
    }

    /// Returns a sub-block source that represents a subrange of the current
    /// block source.
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

    /// Returns a lazy block that represents the current block source that can
    /// be opened on demand.
    pub fn to_lazy_block(&self) -> LazyBlock {
        LazyBlock {
            source: Arc::new(RangeLazyBlockImpl {
                source: self.clone(),
            }),
        }
    }
}

trait LazyBlockImpl {
    fn open(&self) -> ReadResult<Block>;
    fn size(&self) -> Option<u64>;
}

struct RangeLazyBlockImpl {
    source: BlockSource,
}

impl LazyBlockImpl for RangeLazyBlockImpl {
    fn open(&self) -> ReadResult<Block> {
        self.source.open()
    }

    fn size(&self) -> Option<u64> {
        Some(self.source.size())
    }
}

struct FactoryLazyBlockImpl<F>(F);

impl<F> LazyBlockImpl for FactoryLazyBlockImpl<F>
where
    F: Fn() -> ReadResult<Block>,
{
    fn open(&self) -> ReadResult<Block> {
        (self.0)()
    }

    fn size(&self) -> Option<u64> {
        None
    }
}

struct MapLazyBlockImpl<F> {
    base_impl: Arc<dyn LazyBlockImpl>,
    map_fn: F,
}

impl<F> LazyBlockImpl for MapLazyBlockImpl<F>
where
    F: Fn(Block) -> ReadResult<Block>,
{
    fn open(&self) -> ReadResult<Block> {
        let base_block = self.base_impl.open()?;
        (self.map_fn)(base_block)
    }

    fn size(&self) -> Option<u64> {
        None
    }
}

/// A block that is lazily loaded on demand.
///
/// This can be cheaply cloned, but cannot be split into smaller ranges.
#[derive(Clone)]
pub struct LazyBlock {
    source: Arc<dyn LazyBlockImpl>,
}

impl LazyBlock {
    /// Creates a lazy block that is loaded from a factory on demand.
    pub fn from_factory<F>(factory: F) -> Self
    where
        F: Fn() -> ReadResult<Block> + 'static,
    {
        Self {
            source: Arc::new(FactoryLazyBlockImpl(factory)),
        }
    }

    /// Opens a block from the lazy block source. Returns an error if the block
    /// cannot be loaded.
    pub fn open(&self) -> ReadResult<Block> {
        self.source.open()
    }

    /// Creates a new LazyBlock that transforms the result of the current block
    /// with the given function when opened.
    pub fn map<F>(self, map_fn: F) -> Self
    where
        F: Fn(Block) -> ReadResult<Block> + 'static,
    {
        Self {
            source: Arc::new(MapLazyBlockImpl {
                base_impl: self.source,
                map_fn,
            }),
        }
    }

    /// Creates a new lazy block that checks properties about the resulting
    /// block.
    pub fn with_check<F>(&self, check_fn: F) -> Self
    where
        F: Fn(&Block) -> ReadResult<()> + 'static,
    {
        Self {
            source: Arc::new(MapLazyBlockImpl {
                base_impl: self.source.clone(),
                map_fn: move |block| {
                    check_fn(&block)?;
                    Ok(block)
                },
            }),
        }
    }
}

impl std::fmt::Debug for LazyBlock {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("LazyBlock")
            .field("size", &self.source.size())
            .finish()
    }
}

/// A [`DataReader`] that reads from a block.
#[derive(Debug, Clone)]
pub struct BlockReader {
    curr_pos: usize,
    block: Block,
}

impl BlockReader {
    /// Creates a new reader from the block.
    pub fn new(block: Block) -> Self {
        Self { curr_pos: 0, block }
    }

    /// Returns the portion of the block that has not yet been read.
    pub fn into_rest(self) -> Block {
        self.block.sub_buffer(self.curr_pos..)
    }
}

impl DataReader for BlockReader {
    fn read_u8(&mut self) -> io::Result<u8> {
        let mut buf = [0; 1];
        self.block.read_at(self.curr_pos, &mut buf)?;
        self.curr_pos += 1;
        Ok(buf[0])
    }

    fn read_u16_le(&mut self) -> io::Result<u16> {
        let mut buf = [0; 2];
        self.block.read_at(self.curr_pos, &mut buf)?;
        self.curr_pos += 2;
        Ok(u16::from_le_bytes(buf))
    }

    fn read_u24_le(&mut self) -> io::Result<u32> {
        let mut buf = [0; 3];
        self.block.read_at(self.curr_pos, &mut buf)?;
        self.curr_pos += 3;
        Ok(u32::from_le_bytes([buf[0], buf[1], buf[2], 0]))
    }

    fn read_u32_le(&mut self) -> io::Result<u32> {
        let mut buf = [0; 4];
        self.block.read_at(self.curr_pos, &mut buf)?;
        self.curr_pos += 4;
        Ok(u32::from_le_bytes(buf))
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        self.block.read_at(self.curr_pos, buf)?;
        self.curr_pos += buf.len();
        Ok(())
    }

    fn seek_to(&mut self, offset: u32) -> io::Result<()> {
        if offset as usize > self.block.size() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Attempted to seek past the end of the block",
            ));
        }

        self.curr_pos = offset as usize;
        Ok(())
    }

    fn tell(&mut self) -> io::Result<u32> {
        Ok(self.curr_pos as u32)
    }

    fn file_size(&mut self) -> io::Result<u32> {
        Ok(self
            .block
            .size()
            .try_into()
            .map_err(ReadError::from_std_err)?)
    }
}
