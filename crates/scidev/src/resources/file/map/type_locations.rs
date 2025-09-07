use crate::{
    resources::ResourceType,
    utils::mem_reader::{self, MemReader},
};

use super::location_entry::ResourceLocationEntry;

#[derive(Debug)]
pub(super) struct ResourceTypeLocations {
    type_id: ResourceType,
    entries: Vec<ResourceLocationEntry>,
}

impl ResourceTypeLocations {
    pub(super) fn type_id(&self) -> ResourceType {
        self.type_id
    }

    pub(super) fn entries(&self) -> &[ResourceLocationEntry] {
        &self.entries
    }

    pub(crate) fn read_from<M: MemReader>(
        reader: &mut M,
        type_id: ResourceType,
        start: u16,
        end: u16,
    ) -> mem_reader::Result<ResourceTypeLocations, M::Error> {
        // Despite documentation to the contrary, SCI11 uses 5 byte entries in the resource map
        // file.
        assert_eq!((end - start) % 5, 0);
        let count = (end - start) / 5;
        reader.seek_to(usize::from(start))?;
        let mut entries = Vec::new();
        for _ in 0..count {
            entries.push(ResourceLocationEntry::read_from(reader)?);
        }
        Ok(ResourceTypeLocations { type_id, entries })
    }
}
