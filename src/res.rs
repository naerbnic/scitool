use std::{
    collections::{btree_map, BTreeMap},
    fs::File,
    io,
    path::Path,
};

use datafile::DataFile;

use crate::util::block::{Block, BlockReader, BlockSource, LazyBlock};

pub mod audio36;
pub mod datafile;
pub mod mapfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, clap::ValueEnum)]
#[repr(u8)]
pub enum ResourceType {
    View = 0x80,
    Pic,
    Script,
    Text,
    Sound,
    Memory,
    Vocab,
    Font,
    Cursor,
    Patch,
    Bitmap,
    Palette,
    CdAudio,
    Audio,
    Sync,
    Message,
    Map,
    Heap,
    Audio36,
    Sync36,
    Translation,
    Rave,
}

impl TryFrom<u8> for ResourceType {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x80 => Ok(ResourceType::View),
            0x81 => Ok(ResourceType::Pic),
            0x82 => Ok(ResourceType::Script),
            0x83 => Ok(ResourceType::Text),
            0x84 => Ok(ResourceType::Sound),
            0x85 => Ok(ResourceType::Memory),
            0x86 => Ok(ResourceType::Vocab),
            0x87 => Ok(ResourceType::Font),
            0x88 => Ok(ResourceType::Cursor),
            0x89 => Ok(ResourceType::Patch),
            0x8A => Ok(ResourceType::Bitmap),
            0x8B => Ok(ResourceType::Palette),
            0x8C => Ok(ResourceType::CdAudio),
            0x8D => Ok(ResourceType::Audio),
            0x8E => Ok(ResourceType::Sync),
            0x8F => Ok(ResourceType::Message),
            0x90 => Ok(ResourceType::Map),
            0x91 => Ok(ResourceType::Heap),
            0x92 => Ok(ResourceType::Audio36),
            0x93 => Ok(ResourceType::Sync36),
            0x94 => Ok(ResourceType::Translation),
            0x95 => Ok(ResourceType::Rave),
            _ => Err(format!("Invalid resource type: 0x{:02X}", value)),
        }
    }
}

impl From<ResourceType> for u8 {
    fn from(value: ResourceType) -> u8 {
        value as u8
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceId {
    pub type_id: ResourceType,
    pub resource_num: u16,
}

impl ResourceId {
    pub fn new(type_id: ResourceType, resource_num: u16) -> ResourceId {
        ResourceId {
            type_id,
            resource_num,
        }
    }
}

impl std::fmt::Debug for ResourceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}:{:}", self.type_id, self.resource_num)
    }
}

pub struct ResourceSet {
    pub entries: BTreeMap<ResourceId, LazyBlock>,
}

impl ResourceSet {
    pub fn get_resource_block(&self, id: &ResourceId) -> Option<&LazyBlock> {
        self.entries.get(id)
    }

    pub fn resource_ids(&self) -> impl Iterator<Item = &ResourceId> {
        self.entries.keys()
    }

    pub fn with_overlay(&self, overlay: &ResourceSet) -> ResourceSet {
        let mut entries = self.entries.clone();
        for (id, block) in overlay.entries.iter() {
            entries.insert(*id, block.clone());
        }
        ResourceSet { entries }
    }
    
    pub fn merge(&self, other: &ResourceSet) -> io::Result<ResourceSet> {
        let mut entries = self.entries.clone();
        for (id, block) in other.entries.iter() {
            match entries.entry(*id) {
                btree_map::Entry::Vacant(vac) => {
                    vac.insert(block.clone());
                }
                btree_map::Entry::Occupied(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Duplicate resource ID: {:?}", id),
                    ))
                }
            }
        }
        Ok(ResourceSet { entries })
    }
}

pub fn read_resources(map_file: &Path, data_file: &Path) -> io::Result<ResourceSet> {
    let map_file = Block::from_reader(File::open(map_file)?)?;
    let data_file = DataFile::new(BlockSource::from_path(data_file)?);
    let resource_locations = mapfile::ResourceLocations::read_from(BlockReader::new(map_file))?;

    let mut entries = BTreeMap::new();

    for location in resource_locations.locations() {
        let block = data_file.read_contents(&location)?;
        if block.id() != &location.id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Resource ID mismatch: expected {:?}, got {:?}",
                    location.id,
                    block.id()
                ),
            ));
        }
        entries.insert(location.id, block.data().clone());
    }

    Ok(ResourceSet { entries })
}
