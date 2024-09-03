use std::io::{self, Read, Seek};

pub trait DataReader {
    fn read_u8(&mut self) -> io::Result<u8>;
    fn read_u16_le(&mut self) -> io::Result<u16>;
    fn read_u24_le(&mut self) -> io::Result<u32>;
    fn read_u32_le(&mut self) -> io::Result<u32>;
    fn seek_to(&mut self, offset: u32) -> io::Result<()>;
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

    fn seek_to(&mut self, offset: u32) -> io::Result<()> {
        self.0.seek(io::SeekFrom::Start(offset as u64))?;
        Ok(())
    }
}