use std::io;

use super::block::Block;

pub trait DataWriter {
    fn write_u8(&mut self, value: u8) -> io::Result<()>;
    fn write_u16_le(&mut self, value: u16) -> io::Result<()>;
    fn write_u32_le(&mut self, value: u32) -> io::Result<()>;
    fn write_block(&mut self, block: &Block) -> io::Result<()>;
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

    fn write_u32_le(&mut self, value: u32) -> io::Result<()> {
        self.0.write_all(&value.to_le_bytes())
    }

    fn write_block(&mut self, block: &Block) -> io::Result<()> {
        self.0.write_all(&block.read_all()?)
    }
}
