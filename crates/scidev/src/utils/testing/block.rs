use scidev_errors::{diag, ensure, prelude::*};

use crate::utils::{
    block::MemBlock,
    buffer::Buffer,
    mem_reader::{self, BufferMemReader, MemReader, Parse},
};

#[must_use]
pub fn memblock_from_bytes(data: &[u8]) -> MemBlock {
    MemBlock::from_vec(data.to_vec())
}

#[must_use]
pub fn mem_reader_from_bytes(data: &[u8]) -> impl MemReader {
    BufferMemReader::new(memblock_from_bytes(data).into_fallible())
}

pub fn mem_reader_parse_fully<T>(data: impl AsRef<[u8]>) -> Result<T, mem_reader::MemReaderDiag>
where
    T: Parse,
{
    let mut reader = mem_reader_from_bytes(data.as_ref());
    let value = T::parse(&mut reader).raise_err_with(diag!(|| "Failed to parse"))?;
    ensure!(
        reader.tell() == data.as_ref().len(),
        "Expected to parse entire buffer, but {} bytes remain",
        data.as_ref().len() - reader.tell()
    );
    Ok(value)
}
