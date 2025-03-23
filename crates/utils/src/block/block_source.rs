use std::{
    io::{self, Seek},
    ops::RangeBounds,
    path::Path,
    sync::{Arc, Mutex},
};

use super::{LazyBlock, MemBlock, ReadError, ReadResult};

trait GuardImpl {
    fn size(&self) -> usize;
    fn chunk_at(&self, offset: usize) -> &[u8];
}

#[derive(Clone)]
struct GuardPtr<'a> {
    pos: usize,
    guard_impl: Arc<dyn GuardImpl + 'a>,
}

#[derive(Clone)]
enum GuardContents<'a> {
    Slice(&'a [u8]),
    Ptr(GuardPtr<'a>),
}

trait BlockSourceImpl: Send + Sync {
    fn lock(&self, start: u64, size: u64) -> ReadResult<GuardContents<'_>>;
}

struct MemBlockGuard {
    block: MemBlock,
}

impl GuardImpl for MemBlockGuard {
    fn size(&self) -> usize {
        self.block.size()
    }

    fn chunk_at(&self, offset: usize) -> &[u8] {
        assert!(offset < self.block.size());
        &self.block[offset..]
    }
}

struct ReaderBlockSourceImpl<R> {
    reader: Mutex<R>,
}

impl<R> BlockSourceImpl for ReaderBlockSourceImpl<R>
where
    R: io::Read + io::Seek + Send,
{
    fn lock(&self, start: u64, size: u64) -> ReadResult<GuardContents<'_>> {
        let mut reader = self.reader.lock().unwrap();
        reader.seek(io::SeekFrom::Start(start))?;
        let mut data = vec![0; size.try_into().map_err(ReadError::from_std_err)?];
        reader.read_exact(&mut data)?;

        Ok(GuardContents::Ptr(GuardPtr {
            pos: 0,
            guard_impl: Arc::new(MemBlockGuard {
                block: MemBlock::from_vec(data),
            }),
        }))
    }
}

struct VecBlockSourceImpl(Vec<u8>);

impl BlockSourceImpl for VecBlockSourceImpl {
    fn lock(&self, _start: u64, _size: u64) -> ReadResult<GuardContents<'_>> {
        Ok(GuardContents::Slice(&self.0))
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
            source_impl: Arc::new(ReaderBlockSourceImpl {
                reader: Mutex::new(io::BufReader::new(file)),
            }),
        })
    }

    pub fn from_vec(data: Vec<u8>) -> Self {
        let size = data.len() as u64;
        Self {
            start: 0,
            size,
            source_impl: Arc::new(VecBlockSourceImpl(data)),
        }
    }

    /// Returns the size of the block source.
    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn lock(&self) -> ReadResult<Guard<'_>> {
        let guard = self.source_impl.lock(self.start, self.size)?;
        Ok(Guard(guard))
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
    pub fn to_lazy_block(&self) -> LazyBlock {
        LazyBlock::from_block_source(self.clone())
    }
}

#[derive(Clone)]
pub struct Guard<'a>(GuardContents<'a>);

impl bytes::Buf for Guard<'_> {
    fn remaining(&self) -> usize {
        match &self.0 {
            GuardContents::Slice(slice) => slice.len(),
            GuardContents::Ptr(ptr) => ptr.guard_impl.size() - ptr.pos,
        }
    }

    fn chunk(&self) -> &[u8] {
        match &self.0 {
            GuardContents::Slice(slice) => slice,
            GuardContents::Ptr(ptr) => ptr.guard_impl.chunk_at(ptr.pos),
        }
    }

    fn advance(&mut self, cnt: usize) {
        match &mut self.0 {
            GuardContents::Slice(slice) => slice.advance(cnt),
            GuardContents::Ptr(ptr) => ptr.pos += cnt,
        }
    }
}
