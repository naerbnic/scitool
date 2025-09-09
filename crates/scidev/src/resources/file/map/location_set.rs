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
                entry
                    .type_id()
                    .try_into()
                    .map_err(|e| reader.create_invalid_data_error(e))?,
                entry.file_offset(),
                end_offset,
            )?;
            type_locations.push(locations);
        }
        Ok(ResourceLocationSet { type_locations })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{resources::ResourceType, utils::testing::block::mem_reader_from_bytes};
    use datalit::datalit;

    #[test]
    fn test_parse() {
        let map_data = datalit!(
            @endian_mode = le,
            // Index
            0x80,   // Resource View Type
            start(u16, 'res1),
            0x81,   // Resource Picture Type
            start(u16, 'res2),
            0xFF,   // End Marker
            start(u16, 'end), // End Offset

            'res1: {
                // View Locations (5 bytes each)
                1u16,       // Resource Number 1
                1u24, // Offset 0x000002
                2u16,       // Resource Number 2
                3u24, // Offset 0x000006
            },

            'res2: {
                // Picture Locations (5 bytes each)
                1u16,       // Resource Number 1
                4u24, // Offset 0x000008
                2u16,       // Resource Number 2
                5u24, // Offset 0x00000A
            },

            'end: {},

            // Should ignore further data.
            0xDEADBEEF
        );

        let mut reader = mem_reader_from_bytes(&map_data);
        let locations = ResourceLocationSet::parse(&mut reader).unwrap();
        let locations: Vec<_> = locations.locations().collect();
        assert_eq!(locations.len(), 4);
        assert_eq!(locations[0].id(), ResourceId::new(ResourceType::View, 1));
        assert_eq!(locations[0].file_offset(), 2);
        assert_eq!(locations[1].id(), ResourceId::new(ResourceType::View, 2));
        assert_eq!(locations[1].file_offset(), 6);
        assert_eq!(locations[2].id(), ResourceId::new(ResourceType::Pic, 1));
        assert_eq!(locations[2].file_offset(), 8);
        assert_eq!(locations[3].id(), ResourceId::new(ResourceType::Pic, 2));
        assert_eq!(locations[3].file_offset(), 10);
    }
}
