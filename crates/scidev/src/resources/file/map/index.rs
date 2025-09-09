use crate::utils::mem_reader::{self, MemReader, Parse};

use super::index_entry::ResourceIndexEntry;

#[derive(Debug)]
pub(super) struct ResourceIndex {
    entries: Vec<ResourceIndexEntry>,
    end: u16,
}

impl ResourceIndex {
    pub(super) fn entries(&self) -> &[ResourceIndexEntry] {
        &self.entries
    }

    pub(super) fn end(&self) -> u16 {
        self.end
    }
}

impl Parse for ResourceIndex {
    fn parse<M: MemReader>(reader: &mut M) -> mem_reader::Result<Self, M::Error> {
        let mut entries: Vec<ResourceIndexEntry> = Vec::new();
        loop {
            let entry = ResourceIndexEntry::parse(reader)?;
            if let Some(last) = entries.last()
                && entry.file_offset() <= last.file_offset()
            {
                return Err(reader
                    .create_invalid_data_error_msg(
                        "Resource index entries are not in ascending order",
                    )
                    .into());
            }
            if entry.type_id() == 0xFF {
                return Ok(ResourceIndex {
                    entries,
                    end: entry.file_offset(),
                });
            }
            entries.push(entry);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::block::mem_reader_from_bytes;
    use datalit::datalit;

    #[test]
    fn test_parse() {
        let index_data = datalit!(
            0x80,         // Resource View Type
            0x1234u16_le, // Offset 0x1234
            0x81,         // Resource Picture Type
            0x2345u16_le, // Offset 0x2345
            0xFF,         // End Marker
            0x3456u16_le, // End Offset
            // Should ignore further data.
            0xDEADBEEF
        );

        let mut reader = mem_reader_from_bytes(&index_data);
        let index = ResourceIndex::parse(&mut reader).unwrap();
        assert_eq!(index.end(), 0x3456u16);
        let entries = index.entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].type_id(), 0x80);
        assert_eq!(entries[0].file_offset(), 0x1234u16);
        assert_eq!(entries[1].type_id(), 0x81);
        assert_eq!(entries[1].file_offset(), 0x2345u16);
    }
}
