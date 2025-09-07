use crate::{
    resources::ResourceId,
    utils::mem_reader::{self, MemReader, Parse},
};

use super::{
    index::ResourceIndex, index_entry::ResourceIndexEntry, type_locations::ResourceTypeLocations,
};

/// The location of a resource within a resource data file
#[derive(Debug, Clone, Copy)]
pub(crate) struct ResourceLocation {
    id: ResourceId,
    file_offset: u32,
}

impl ResourceLocation {
    pub(crate) fn new(id: ResourceId, file_offset: u32) -> Self {
        ResourceLocation { id, file_offset }
    }

    pub(crate) fn id(self) -> ResourceId {
        self.id
    }

    pub(crate) fn file_offset(self) -> u32 {
        self.file_offset
    }
}

#[derive(Debug)]
pub(crate) struct ResourceLocationSet {
    type_locations: Vec<ResourceTypeLocations>,
}

impl ResourceLocationSet {
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

impl Parse for ResourceLocationSet {
    fn parse<M: MemReader>(reader: &mut M) -> mem_reader::Result<Self, M::Error> {
        let index = ResourceIndex::parse(reader)?;
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
}
