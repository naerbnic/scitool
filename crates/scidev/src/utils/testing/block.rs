use crate::utils::{
    block::MemBlock,
    errors::NoError,
    mem_reader::{BufferMemReader, MemReader},
};

#[must_use]
pub fn memblock_from_bytes(data: &[u8]) -> MemBlock {
    MemBlock::from_vec(data.to_vec())
}

#[must_use]
pub fn mem_reader_from_bytes(data: &[u8]) -> impl MemReader<Error = NoError> {
    BufferMemReader::new(memblock_from_bytes(data))
}
