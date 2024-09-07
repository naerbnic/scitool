use std::io;

use super::{block::Block, data_block::WriteBlock};

pub trait DataWriter {
    fn write_u8(&mut self, value: u8) -> io::Result<()>;
    fn write_u16_le(&mut self, value: u16) -> io::Result<()>;
    fn write_u24_le(&mut self, value: u32) -> io::Result<()>;
    fn write_u32_le(&mut self, value: u32) -> io::Result<()>;
    fn write_block(&mut self, block: &Block) -> io::Result<()>;
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()>;
    // Seek to the given offset in the file. If the offset is beyond the end of the file, the file
    // will be extended with zeroes.
    fn seek_to(&mut self, offset: u32) -> io::Result<()>;
}

fn write_zeroes<W: io::Write>(writer: &mut W, mut count: usize) -> io::Result<()> {
    static ZEROES: [u8; 1024] = [0; 1024];
    while count > ZEROES.len() {
        writer.write_all(&ZEROES)?;
        count -= ZEROES.len();
    }
    writer.write_all(&ZEROES[0..count])?;
    Ok(())
}

pub struct IoDataWriter<W>(W);

impl<W: io::Write + io::Seek> IoDataWriter<W> {
    pub fn new(writer: W) -> IoDataWriter<W> {
        IoDataWriter(writer)
    }
}

impl<W: io::Write + io::Seek> DataWriter for IoDataWriter<W> {
    fn write_u8(&mut self, value: u8) -> io::Result<()> {
        self.0.write_all(&[value])
    }

    fn write_u16_le(&mut self, value: u16) -> io::Result<()> {
        self.0.write_all(&value.to_le_bytes())
    }

    fn write_u24_le(&mut self, value: u32) -> io::Result<()> {
        self.0.write_all(&value.to_le_bytes()[0..3])
    }

    fn write_u32_le(&mut self, value: u32) -> io::Result<()> {
        self.0.write_all(&value.to_le_bytes())
    }

    fn write_block(&mut self, block: &Block) -> io::Result<()> {
        self.0.write_all(&block.read_all()?)
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.0.write_all(buf)
    }

    fn seek_to(&mut self, offset: u32) -> io::Result<()> {
        let file_length = self.0.seek(io::SeekFrom::End(0))?;
        if file_length < offset as u64 {
            self.0.seek(io::SeekFrom::Start(file_length))?;
            write_zeroes(&mut self.0, (offset as usize) - file_length as usize)?;
        } else {
            self.0.seek(io::SeekFrom::Start(offset as u64))?;
        }
        Ok(())
    }
}

pub struct TargetWriter<T> {
    target: T,
    position: u64,
}

impl<T> DataWriter for TargetWriter<T>
where
    T: WriteBlock,
{
    fn write_u8(&mut self, value: u8) -> io::Result<()> {
        self.target.write_at(self.position, &[value])?;
        self.position += 1;
        Ok(())
    }

    fn write_u16_le(&mut self, value: u16) -> io::Result<()> {
        self.target.write_at(self.position, &value.to_le_bytes())?;
        self.position += 2;
        Ok(())
    }

    fn write_u24_le(&mut self, value: u32) -> io::Result<()> {
        self.target
            .write_at(self.position, &value.to_le_bytes()[0..3])?;
        self.position += 3;
        Ok(())
    }

    fn write_u32_le(&mut self, value: u32) -> io::Result<()> {
        self.target.write_at(self.position, &value.to_le_bytes())?;
        self.position += 4;
        Ok(())
    }

    fn write_block(&mut self, block: &Block) -> io::Result<()> {
        self.write_all(&block.read_all()?)
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.target.write_at(self.position, buf)?;
        self.position += buf.len() as u64;
        Ok(())
    }

    fn seek_to(&mut self, offset: u32) -> io::Result<()> {
        self.position = offset as u64;
        Ok(())
    }
}
