use std::{
    collections::{btree_map, BTreeMap},
    fs::File,
    io,
    path::Path,
};

use data::DataFile;

use sci_utils::block::{Block, BlockReader, BlockSource, LazyBlock};

use super::{ResourceId, ResourceType};

mod data;
mod map;

pub fn read_resources(map_file: &Path, data_file: &Path) -> io::Result<ResourceSet> {
    let map_file = Block::from_reader(File::open(map_file)?)?;
    let data_file = DataFile::new(BlockSource::from_path(data_file)?);
    let resource_locations = map::ResourceLocations::read_from(BlockReader::new(map_file))?;

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

pub struct ResourceSet {
    pub entries: BTreeMap<ResourceId, LazyBlock>,
}

impl ResourceSet {
    pub fn get_resource(&self, id: &ResourceId) -> Option<Resource> {
        self.entries.get(id).map(|b| Resource {
            id: *id,
            source: b.clone(),
        })
    }

    pub fn resource_ids(&self) -> impl Iterator<Item = ResourceId> + '_ {
        self.entries.keys().copied()
    }

    pub fn resources(&self) -> impl Iterator<Item = Resource> + '_ {
        self.entries.iter().map(|(id, block)| Resource {
            id: *id,
            source: block.clone(),
        })
    }

    pub fn resources_of_type(&self, type_id: ResourceType) -> impl Iterator<Item = Resource> + '_ {
        self.entries.iter().filter_map(move |(id, block)| {
            if id.type_id != type_id {
                return None;
            }
            Some(Resource {
                id: *id,
                source: block.clone(),
            })
        })
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

pub fn open_game_resources(root_dir: &Path) -> anyhow::Result<ResourceSet> {
    let main_set = {
        let map_file = root_dir.join("RESOURCE.MAP");
        let data_file = root_dir.join("RESOURCE.000");
        read_resources(&map_file, &data_file)?
    };

    let message_set = {
        let map_file = root_dir.join("MESSAGE.MAP");
        let data_file = root_dir.join("RESOURCE.MSG");
        read_resources(&map_file, &data_file)?
    };
    Ok(main_set.merge(&message_set)?)
}

pub struct Resource {
    id: ResourceId,
    source: LazyBlock,
}

impl Resource {
    pub fn id(&self) -> &ResourceId {
        &self.id
    }

    pub fn load_data(&self) -> anyhow::Result<Block> {
        Ok(self.source.open()?)
    }
}
