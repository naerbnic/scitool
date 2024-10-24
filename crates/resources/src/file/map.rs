use std::io;

use crate::{ResourceId, ResourceType};
use sci_utils::data_reader::DataReader;

#[derive(Debug)]
pub struct ResourceIndexEntry {
    pub type_id: u8,
    pub file_offset: u16,
}

impl ResourceIndexEntry {
    pub fn read_from<R: DataReader>(mut reader: R) -> io::Result<ResourceIndexEntry> {
        let type_id = reader.read_u8()?;
        let file_offset = reader.read_u16_le()?;
        Ok(ResourceIndexEntry {
            type_id,
            file_offset,
        })
    }
}

#[derive(Debug)]
pub struct ResourceIndex {
    pub entries: Vec<ResourceIndexEntry>,
    pub end: u16,
}

impl ResourceIndex {
    pub fn read_from<R: DataReader>(mut reader: R) -> io::Result<ResourceIndex> {
        let mut entries = Vec::new();
        loop {
            let entry = ResourceIndexEntry::read_from(&mut reader)?;
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
pub struct ResourceLocationEntry {
    pub resource_num: u16,
    pub resource_file_offset: u32,
}

impl ResourceLocationEntry {
    pub fn read_from<R: DataReader>(reader: &mut R) -> io::Result<ResourceLocationEntry> {
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
pub struct ResourceTypeLocations {
    pub type_id: ResourceType,
    pub entries: Vec<ResourceLocationEntry>,
}

impl ResourceTypeLocations {
    pub fn read_from<R: DataReader>(
        reader: &mut R,
        type_id: ResourceType,
        start: u16,
        end: u16,
    ) -> io::Result<ResourceTypeLocations> {
        // Despite documentation to the contrary, SCI11 uses 5 byte entries in the resource map
        // file.
        assert_eq!((end - start) % 5, 0);
        let count = (end - start) / 5;
        reader.seek_to(start as u32)?;
        let mut entries = Vec::new();
        for _ in 0..count {
            entries.push(ResourceLocationEntry::read_from(reader)?);
        }
        Ok(ResourceTypeLocations { type_id, entries })
    }
}

#[derive(Debug)]
pub struct ResourceLocations {
    pub type_locations: Vec<ResourceTypeLocations>,
}

impl ResourceLocations {
    pub fn read_from<R: DataReader>(mut reader: R) -> io::Result<ResourceLocations> {
        let index = ResourceIndex::read_from(&mut reader)?;
        let mut type_locations = Vec::new();

        let end_offsets = index
            .entries
            .iter()
            .map(|entry| entry.file_offset)
            .skip(1)
            .chain(std::iter::once(index.end));
        for (entry, end_offset) in index.entries.iter().zip(end_offsets) {
            let locations = ResourceTypeLocations::read_from(
                &mut reader,
                entry.type_id.try_into().unwrap(),
                entry.file_offset,
                end_offset,
            )?;
            type_locations.push(locations);
        }
        Ok(ResourceLocations { type_locations })
    }

    pub fn locations(&self) -> impl Iterator<Item = ResourceLocation> + '_ {
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
pub struct ResourceLocation {
    pub id: ResourceId,
    pub file_offset: u32,
}
