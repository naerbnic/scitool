use std::io::{self, Read, Seek};

use super::block::{BlockReader, BlockSource};

pub trait DataReader {
    fn read_u8(&mut self) -> io::Result<u8>;
    fn read_u16_le(&mut self) -> io::Result<u16>;
    fn read_u24_le(&mut self) -> io::Result<u32>;
    fn read_u32_le(&mut self) -> io::Result<u32>;
    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()>;
    fn seek_to(&mut self, offset: u32) -> io::Result<()>;
    fn tell(&mut self) -> io::Result<u32>;
    fn file_size(&mut self) -> io::Result<u32>;
}

impl<R> DataReader for &mut R
where
    R: DataReader,
{
    fn read_u8(&mut self) -> io::Result<u8> {
        (**self).read_u8()
    }

    fn read_u16_le(&mut self) -> io::Result<u16> {
        (**self).read_u16_le()
    }

    fn read_u24_le(&mut self) -> io::Result<u32> {
        (**self).read_u24_le()
    }

    fn read_u32_le(&mut self) -> io::Result<u32> {
        (**self).read_u32_le()
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        (**self).read_exact(buf)
    }

    fn seek_to(&mut self, offset: u32) -> io::Result<()> {
        (**self).seek_to(offset)
    }

    fn tell(&mut self) -> io::Result<u32> {
        (**self).tell()
    }

    fn file_size(&mut self) -> io::Result<u32> {
        (**self).file_size()
    }
}

impl<T> DataReader for Box<T>
where
    T: DataReader + ?Sized,
{
    fn read_u8(&mut self) -> io::Result<u8> {
        (**self).read_u8()
    }

    fn read_u16_le(&mut self) -> io::Result<u16> {
        (**self).read_u16_le()
    }

    fn read_u24_le(&mut self) -> io::Result<u32> {
        (**self).read_u24_le()
    }

    fn read_u32_le(&mut self) -> io::Result<u32> {
        (**self).read_u32_le()
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        (**self).read_exact(buf)
    }

    fn seek_to(&mut self, offset: u32) -> io::Result<()> {
        (**self).seek_to(offset)
    }

    fn tell(&mut self) -> io::Result<u32> {
        (**self).tell()
    }

    fn file_size(&mut self) -> io::Result<u32> {
        (**self).file_size()
    }
}
pub struct IoDataReader<R>(R);

impl<R: Read + Seek> IoDataReader<R> {
    pub fn new(reader: R) -> IoDataReader<R> {
        IoDataReader(reader)
    }
}

impl<R: Read + Seek> DataReader for IoDataReader<R> {
    fn read_u8(&mut self) -> io::Result<u8> {
        let mut buf = [0; 1];
        self.0.read_exact(&mut buf)?;
        Ok(buf[0])
    }

    fn read_u16_le(&mut self) -> io::Result<u16> {
        let mut buf = [0; 2];
        self.0.read_exact(&mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }

    fn read_u24_le(&mut self) -> io::Result<u32> {
        let mut buf = [0; 4];
        self.0.read_exact(&mut buf[0..3])?;
        Ok(u32::from_le_bytes([buf[0], buf[1], buf[2], 0]))
    }

    fn read_u32_le(&mut self) -> io::Result<u32> {
        let mut buf = [0; 4];
        self.0.read_exact(&mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        self.0.read_exact(buf)
    }

    fn seek_to(&mut self, offset: u32) -> io::Result<()> {
        self.0.seek(io::SeekFrom::Start(offset as u64))?;
        Ok(())
    }

    fn tell(&mut self) -> io::Result<u32> {
        Ok(self.0.stream_position()?.try_into().unwrap())
    }

    fn file_size(&mut self) -> io::Result<u32> {
        let curr_offset = self.tell()?;
        let result = self.0.seek(io::SeekFrom::End(0))?.try_into().unwrap();
        self.seek_to(curr_offset)?;
        Ok(result)
    }
}

pub trait FromBlockSource: Sized {
    fn from_block_source(source: &BlockSource) -> io::Result<(Self, BlockSource)> {
        let block = source
            .subblock(..Self::read_size() as u64)
            .open()?;
        let header = Self::parse(BlockReader::new(block))?;
        let rest = source.subblock(Self::read_size() as u64..);
        Ok((header, rest))
    }

    fn read_size() -> usize;

    fn parse<R>(reader: R) -> io::Result<Self>
    where
        R: DataReader;
}
