use std::io;

use crate::utils::mem_reader::{MemReader, Parse};

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
    fn parse<M: MemReader>(reader: &mut M) -> io::Result<Self> {
        let type_id = reader.read_u8()?;
        let file_offset = reader.read_u16_le()?;
        Ok(ResourceIndexEntry {
            type_id,
            file_offset,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::testing::block::mem_reader_from_bytes;
    use datalit::datalit;

    use super::*;

    #[test]
    fn test_parse() {
        let data = datalit!(
            0x01,         // Resource View Type
            0x1234u16_le, // Offset 0x1234
        );
        let mut reader = mem_reader_from_bytes(data);
        let entry = ResourceIndexEntry::parse(&mut reader).unwrap();
        assert_eq!(entry.type_id(), 1);
        assert_eq!(entry.file_offset(), 0x1234);
    }
}
