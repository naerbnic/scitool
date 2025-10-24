use std::io;

use crate::utils::{
    block::{
        MemBlock,
        block2::{Block, BlockBase},
    },
    range::BoundedRange,
};

#[derive(Debug)]
pub(super) struct SequenceBlockImpl {
    blocks: Vec<Block>,
}

impl SequenceBlockImpl {
    pub(super) fn new(blocks: impl IntoIterator<Item = Block>) -> Self {
        Self {
            blocks: blocks.into_iter().collect(),
        }
    }

    pub(super) fn size(&self) -> u64 {
        self.blocks.iter().map(Block::len).sum()
    }
}

impl BlockBase for SequenceBlockImpl {
    fn open_mem(&self, range: BoundedRange<u64>) -> io::Result<MemBlock> {
        let mut data = Vec::new();
        let mut remaining_range = range;
        let mut iter = self.blocks.iter();
        while remaining_range.size() > 0
            && let Some(curr_block) = iter.next()
        {
            if let Some(curr_range) = remaining_range.intersect(0..curr_block.len()) {
                data.push(curr_block.open_mem(curr_range)?);
            }
            remaining_range = remaining_range.shift_down_by(curr_block.len());
        }
        Ok(MemBlock::concat_blocks(data))
    }

    fn open_reader<'a>(&'a self, range: BoundedRange<u64>) -> io::Result<Box<dyn io::Read + 'a>> {
        struct SequenceReader<'a> {
            remaining_size: u64,
            remaining_blocks: &'a [Block],
            current_reader: Option<Box<dyn io::Read + 'a>>,
        }

        impl io::Read for SequenceReader<'_> {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                if self.remaining_size == 0 {
                    self.current_reader = None;
                    self.remaining_blocks = &[];
                    return Ok(0);
                }
                loop {
                    let reader = if let Some(r) = &mut self.current_reader {
                        r
                    } else {
                        let Some((next_block, remaining)) = self.remaining_blocks.split_first()
                        else {
                            return Err(io::Error::new(
                                io::ErrorKind::UnexpectedEof,
                                "no more blocks to read",
                            ));
                        };
                        self.remaining_blocks = remaining;
                        self.current_reader = Some(
                            next_block.open_reader(BoundedRange::from_size(next_block.len()))?,
                        );
                        self.current_reader.as_mut().unwrap()
                    };
                    let to_read = std::cmp::min(buf.len().try_into().unwrap(), self.remaining_size)
                        .try_into()
                        .unwrap();
                    let read_bytes = reader.read(&mut buf[..to_read])?;
                    if read_bytes != 0 {
                        self.remaining_size -= read_bytes as u64;
                        return Ok(read_bytes);
                    }
                    self.current_reader = None;
                }
            }
        }

        if range.size() == 0 {
            return Ok(Box::new(io::empty()));
        }

        let mut remaining_range = range;
        let mut blocks = &self.blocks[..];

        let first_block = loop {
            if remaining_range.size() == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "range extends beyond end of sequence block",
                ));
            }
            let Some((first_block, rest)) = blocks.split_first() else {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "range extends beyond end of sequence block",
                ));
            };
            blocks = rest;
            if remaining_range.start() < first_block.len() {
                break first_block;
            }
            remaining_range = remaining_range.shift_down_by(first_block.len());
        };

        let initial_reader =
            first_block.open_reader(remaining_range.intersect(0..first_block.len()).unwrap())?;

        Ok(Box::new(SequenceReader {
            remaining_size: remaining_range.size(),
            remaining_blocks: blocks,
            current_reader: Some(initial_reader),
        }))
    }
}
