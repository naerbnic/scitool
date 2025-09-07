use crate::utils::mem_reader::{self, MemReader, Parse};

#[derive(Debug)]
pub(super) struct ResourceIndexEntry {
    type_id: u8,
    file_offset: u16,
}

impl ResourceIndexEntry {
    pub(super) fn type_id(&self) -> u8 {
        self.type_id
    }

    pub(super) fn file_offset(&self) -> u16 {
        self.file_offset
    }
}

impl Parse for ResourceIndexEntry {
    fn parse<M: MemReader>(reader: &mut M) -> mem_reader::Result<Self, M::Error> {
        let type_id = reader.read_u8()?;
        let file_offset = reader.read_u16_le()?;
        Ok(ResourceIndexEntry {
            type_id,
            file_offset,
        })
    }
}
