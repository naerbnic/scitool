use std::{
    any::Any, io::{self, Seek}, ops::RangeBounds, path::Path, sync::{Arc, Mutex}
};

use super::data_reader::DataReader;

#[derive(thiserror::Error)]
#[error(transparent)]
pub struct ReadError(io::Error);

impl std::fmt::Debug for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.0, f)
    }
}

impl ReadError {
    pub fn new(err: io::Error) -> Self {
        Self(err)
    }

    pub fn from_std_err<E>(err: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        // This should get optimized away.
        match (Box::new(err) as Box<dyn Any>).downcast::<io::Error>() {
            Ok(io_err) => Self(*io_err),
            Err(err) => Self(io::Error::new(
                io::ErrorKind::Other,
                err.downcast::<E>().unwrap(),
            )),
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

pub type ReadResult<T> = std::result::Result<T, ReadError>;

#[derive(Clone)]
pub struct Block {
    start: u64,
    size: u64,
    data: Arc<Vec<u8>>,
}

impl std::fmt::Debug for Block {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_tuple("Block")
            .field(&&self.data[self.start as usize..][..self.size as usize])
            .finish()
    }
}

impl Block {
    pub fn from_vec(data: Vec<u8>) -> Self {
        let size = data.len() as u64;
        Self {
            start: 0,
            size,
            data: Arc::new(data),
        }
    }

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

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn read_at(&self, offset: u64, buf: &mut [u8]) -> ReadResult<()> {
        if offset + buf.len() as u64 > self.size {
            return Err(ReadError::new(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Attempted to read past the end of the block",
            )));
        }

        buf.copy_from_slice(&self.data[(self.start + offset) as usize..][..buf.len()]);
        Ok(())
    }

    pub fn read_all(&self) -> ReadResult<Vec<u8>> {
        let mut buf = vec![0; self.size.try_into().map_err(ReadError::from_std_err)?];
        self.read_at(0, &mut buf)?;
        Ok(buf)
    }

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
            data: self.data.clone(),
        }
    }
}

pub trait BlockSourceImpl {
    fn read_block(&self, start: u64, size: u64) -> ReadResult<Block>;
}

struct ReaderBlockSourceImpl<R>(Mutex<R>);

impl<R> BlockSourceImpl for ReaderBlockSourceImpl<R>
where
    R: io::Read + io::Seek,
{
    fn read_block(&self, start: u64, size: u64) -> ReadResult<Block> {
        let mut reader = self.0.lock().unwrap();
        reader.seek(io::SeekFrom::Start(start))?;
        let mut data = vec![0; size.try_into().map_err(ReadError::from_std_err)?];
        reader.read_exact(&mut data)?;

        Ok(Block::from_vec(data))
    }
}

pub struct BlockSource {
    start: u64,
    size: u64,
    source_impl: Arc<dyn BlockSourceImpl>,
}

impl BlockSource {
    pub fn from_path(path: &Path) -> io::Result<Self> {
        let mut file = std::fs::File::open(&path)?;
        let size = file.seek(io::SeekFrom::End(0))?;
        Ok(Self {
            start: 0,
            size,
            source_impl: Arc::new(ReaderBlockSourceImpl(Mutex::new(io::BufReader::new(file)))),
        })
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn open(&self) -> ReadResult<Block> {
        self.source_impl.read_block(self.start, self.size)
    }

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
}

#[derive(Debug, Clone)]
pub struct BlockReader {
    curr_pos: u64,
    block: Block,
}

impl BlockReader {
    pub fn new(block: Block) -> Self {
        Self { curr_pos: 0, block }
    }

    pub fn into_rest(self) -> Block {
        self.block.subblock(self.curr_pos..)
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
        self.curr_pos += buf.len() as u64;
        Ok(())
    }

    fn seek_to(&mut self, offset: u32) -> io::Result<()> {
        if offset as u64 > self.block.size() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Attempted to seek past the end of the block",
            ));
        }

        self.curr_pos = offset as u64;
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
