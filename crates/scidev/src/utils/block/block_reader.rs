use std::io;

use crate::utils::{buffer::BufferExt, data_reader::DataReader};

use super::{MemBlock, ReadError};

/// A [`DataReader`] that reads from a block.
#[derive(Debug, Clone)]
pub struct BlockReader {
    curr_pos: usize,
    block: MemBlock,
}

impl BlockReader {
    /// Creates a new reader from the block.
    #[must_use]
    pub fn new(block: MemBlock) -> Self {
        Self { curr_pos: 0, block }
    }

    /// Returns the portion of the block that has not yet been read.
    #[must_use]
    pub fn into_rest(self) -> MemBlock {
        let curr_pos: u64 = self.curr_pos.try_into().unwrap();
        self.block.sub_buffer(curr_pos..)
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
        #[expect(clippy::cast_possible_truncation)]
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
