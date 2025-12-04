use crate::utils::{
    block::MemBlock,
    buffer::Buffer,
    errors::OtherError,
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

pub fn mem_reader_parse_fully<T: Parse>(data: impl AsRef<[u8]>) -> mem_reader::Result<T> {
    let mut reader = mem_reader_from_bytes(data.as_ref());
    let value = T::parse(&mut reader)?;
    if reader.tell() != data.as_ref().len() {
        return Err(reader
            .create_invalid_data_error(OtherError::from_msg(format!(
                "Expected to parse entire buffer, but {} bytes remain",
                data.as_ref().len() - reader.tell()
            )))
            .into());
    }
    Ok(value)
}
