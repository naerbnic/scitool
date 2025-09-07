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
        let mut entries = Vec::new();
        loop {
            let entry = ResourceIndexEntry::parse(reader)?;
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
