use std::io;

use crate::utils::{
    block::{MemBlock, block2::BlockBase},
    range::BoundedRange,
};

pub(super) struct EmptyBlockImpl;

impl BlockBase for EmptyBlockImpl {
    fn open_mem(&self, range: BoundedRange<u64>) -> io::Result<MemBlock> {
        if range.size() > 0 {
            Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "cannot read from empty block",
            ))
        } else {
            Ok(MemBlock::empty())
        }
    }

    fn open_reader<'a>(&'a self, range: BoundedRange<u64>) -> io::Result<Box<dyn io::Read + 'a>> {
        if range.size() > 0 {
            Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "cannot read from empty block",
            ))
        } else {
            Ok(Box::new(io::empty()))
        }
    }
}
