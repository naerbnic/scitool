use std::{
    collections::{btree_map, BTreeMap},
    fs::File,
    io,
    path::Path,
};

use data::DataFile;

use crate::util::block::{Block, BlockReader, BlockSource, LazyBlock};

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
    pub fn get_resource_block(&self, id: &ResourceId) -> Option<&LazyBlock> {
        self.entries.get(id)
    }

    pub fn resource_ids(&self) -> impl Iterator<Item = &ResourceId> {
        self.entries.keys()
    }

    #[expect(dead_code)]
    pub fn resources(&self) -> impl Iterator<Item = (&ResourceId, &LazyBlock)> {
        self.entries.iter()
    }

    pub fn resources_of_type(
        &self,
        type_id: ResourceType,
    ) -> impl Iterator<Item = (&ResourceId, &LazyBlock)> {
        self.entries
            .iter()
            .filter(move |(id, _)| id.type_id == type_id)
    }

    #[expect(dead_code)]
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
