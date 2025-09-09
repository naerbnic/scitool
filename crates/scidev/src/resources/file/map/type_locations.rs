use crate::{
    resources::ResourceType,
    utils::mem_reader::{self, MemReader, Parse},
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
        if (end - start) % 5 != 0 {
            return Err(reader
                .create_invalid_data_error_msg(format!(
                    "Resource type {type_id:?} has invalid location entry size: {} bytes",
                    end - start
                ))
                .into());
        }
        let count = (end - start) / 5;
        reader.seek_to(usize::from(start))?;
        let mut entries = Vec::new();
        for _ in 0..count {
            entries.push(ResourceLocationEntry::read_from(reader)?);
        }
        Ok(ResourceTypeLocations { type_id, entries })
    }
}

impl Parse for ResourceTypeLocations {
    fn parse<M: MemReader>(_: &mut M) -> mem_reader::Result<Self, M::Error> {
        unimplemented!("ResourceTypeLocations cannot be parsed without additional context")
    }
}
