use std::io::{self, Read, Seek};

use super::data_source::DataSource;

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

pub struct SourceReader<S> {
    source: S,
    curr_pos: u64,
}

impl<S> SourceReader<S>
where
    S: DataSource,
{
    pub fn new(source: S) -> Self {
        Self {
            source,
            curr_pos: 0,
        }
    }
}

impl<S> DataReader for SourceReader<S>
where
    S: DataSource,
{
    fn read_u8(&mut self) -> io::Result<u8> {
        let mut buf = [0; 1];
        self.source.read_at(self.curr_pos, &mut buf)?;
        self.curr_pos += 1;
        Ok(buf[0])
    }

    fn read_u16_le(&mut self) -> io::Result<u16> {
        let mut buf = [0; 2];
        self.source.read_at(self.curr_pos, &mut buf)?;
        self.curr_pos += 2;
        Ok(u16::from_le_bytes(buf))
    }

    fn read_u24_le(&mut self) -> io::Result<u32> {
        let mut buf = [0; 3];
        self.source.read_at(self.curr_pos, &mut buf)?;
        self.curr_pos += 3;
        Ok(u32::from_le_bytes([buf[0], buf[1], buf[2], 0]))
    }

    fn read_u32_le(&mut self) -> io::Result<u32> {
        let mut buf = [0; 4];
        self.source.read_at(self.curr_pos, &mut buf)?;
        self.curr_pos += 4;
        Ok(u32::from_le_bytes(buf))
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        self.source.read_at(self.curr_pos, buf)?;
        self.curr_pos += buf.len() as u64;
        Ok(())
    }

    fn seek_to(&mut self, offset: u32) -> io::Result<()> {
        if offset as u64 > self.source.size()? {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Attempted to seek past the end of the data source",
            ));
        }
        self.curr_pos = offset as u64;
        Ok(())
    }

    fn tell(&mut self) -> io::Result<u32> {
        Ok(self.curr_pos.try_into().unwrap())
    }

    fn file_size(&mut self) -> io::Result<u32> {
        Ok(self.source.size()? as u32)
    }
}
