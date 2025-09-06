//! Types for reading the resource map file.

use crate::{
    resources::{ResourceId, ResourceType},
    utils::mem_reader::{self, MemReader},
};

#[derive(Debug)]
pub(crate) struct ResourceIndexEntry {
    pub type_id: u8,
    pub file_offset: u16,
}

impl ResourceIndexEntry {
    pub(crate) fn read_from<M: MemReader>(
        reader: &mut M,
    ) -> mem_reader::Result<ResourceIndexEntry, M::Error> {
        let type_id = reader.read_u8()?;
        let file_offset = reader.read_u16_le()?;
        Ok(ResourceIndexEntry {
            type_id,
            file_offset,
        })
    }
}

#[derive(Debug)]
pub(crate) struct ResourceIndex {
    pub entries: Vec<ResourceIndexEntry>,
    pub end: u16,
}

impl ResourceIndex {
    pub(crate) fn read_from<M: MemReader>(
        reader: &mut M,
    ) -> mem_reader::Result<ResourceIndex, M::Error> {
        let mut entries = Vec::new();
        loop {
            let entry = ResourceIndexEntry::read_from(reader)?;
            if entry.type_id == 0xFF {
                return Ok(ResourceIndex {
                    entries,
                    end: entry.file_offset,
                });
            }
            entries.push(entry);
        }
    }
}

#[derive(Debug)]
pub(crate) struct ResourceLocationEntry {
    pub resource_num: u16,
    pub resource_file_offset: u32,
}

impl ResourceLocationEntry {
    pub(crate) fn read_from<M: MemReader>(
        reader: &mut M,
    ) -> mem_reader::Result<ResourceLocationEntry, M::Error> {
        let resource_num = reader.read_u16_le()?;
        let body = reader.read_u24_le()?;
        assert_eq!(body & 0xF000_0000, 0);
        let resource_file_offset = (body & 0x0FFF_FFFF) << 1;
        Ok(ResourceLocationEntry {
            resource_num,
            resource_file_offset,
        })
    }
}

#[derive(Debug)]
pub(crate) struct ResourceTypeLocations {
    pub type_id: ResourceType,
    pub entries: Vec<ResourceLocationEntry>,
}

impl ResourceTypeLocations {
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

#[derive(Debug)]
pub(crate) struct ResourceLocations {
    pub type_locations: Vec<ResourceTypeLocations>,
}

impl ResourceLocations {
    pub(crate) fn read_from<M: MemReader>(
        reader: &mut M,
    ) -> mem_reader::Result<ResourceLocations, M::Error> {
        let index = ResourceIndex::read_from(reader)?;
        let mut type_locations = Vec::new();

        let end_offsets = index
            .entries
            .iter()
            .map(|entry| entry.file_offset)
            .skip(1)
            .chain(std::iter::once(index.end));
        for (entry, end_offset) in index.entries.iter().zip(end_offsets) {
            let locations = ResourceTypeLocations::read_from(
                reader,
                entry.type_id.try_into().unwrap(),
                entry.file_offset,
                end_offset,
            )?;
            type_locations.push(locations);
        }
        Ok(ResourceLocations { type_locations })
    }

    pub(crate) fn locations(&self) -> impl Iterator<Item = ResourceLocation> + '_ {
        self.type_locations.iter().flat_map(|locations| {
            locations.entries.iter().map(move |entry| ResourceLocation {
                id: ResourceId::new(locations.type_id, entry.resource_num),
                file_offset: entry.resource_file_offset,
            })
        })
    }
}

/// The location of a resource within a resource data file
#[derive(Debug, Clone, Copy)]
pub(crate) struct ResourceLocation {
    pub id: ResourceId,
    pub file_offset: u32,
}
