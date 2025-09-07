use crate::{
    resources::ResourceId,
    utils::mem_reader::{self, MemReader},
};

use super::{
    ResourceLocation, index::ResourceIndex, index_entry::ResourceIndexEntry,
    type_locations::ResourceTypeLocations,
};

#[derive(Debug)]
pub(crate) struct ResourceLocationSet {
    type_locations: Vec<ResourceTypeLocations>,
}

impl ResourceLocationSet {
    pub(crate) fn read_from<M: MemReader>(
        reader: &mut M,
    ) -> mem_reader::Result<ResourceLocationSet, M::Error> {
        let index = ResourceIndex::read_from(reader)?;
        let mut type_locations = Vec::new();

        let end_offsets = index
            .entries()
            .iter()
            .map(ResourceIndexEntry::file_offset)
            .skip(1)
            .chain(std::iter::once(index.end()));
        for (entry, end_offset) in index.entries().iter().zip(end_offsets) {
            let locations = ResourceTypeLocations::read_from(
                reader,
                entry.type_id().try_into().unwrap(),
                entry.file_offset(),
                end_offset,
            )?;
            type_locations.push(locations);
        }
        Ok(ResourceLocationSet { type_locations })
    }

    pub(crate) fn locations(&self) -> impl Iterator<Item = ResourceLocation> + '_ {
        self.type_locations.iter().flat_map(|locations| {
            locations.entries().iter().map(move |entry| {
                ResourceLocation::new(
                    ResourceId::new(locations.type_id(), entry.resource_num()),
                    entry.resource_file_offset(),
                )
            })
        })
    }
}
