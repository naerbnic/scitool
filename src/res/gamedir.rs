use crate::util::data_block::{self, path::PathBlockSource, BlockSource, ReadBlock};

pub struct ResourceFile {
    resource_map: Box<dyn BlockSource>,
    resource_data: Box<dyn BlockSource>,
}

impl ResourceFile {
    pub fn new(resource_map: Box<dyn BlockSource>, resource_data: Box<dyn BlockSource>) -> Self {
        ResourceFile {
            resource_map,
            resource_data,
        }
    }

    pub fn open(&self) -> data_block::Result<ResourceFileSource> {
        Ok(ResourceFileSource {
            map_block: self.resource_map.open_read()?,
            data_block: self.resource_data.open_read()?,
        })
    }
}

pub struct ResourceFileSource {
    map_block: Box<dyn ReadBlock>,
    data_block: Box<dyn ReadBlock>,
}

pub struct GameDir {
    root_path: std::path::PathBuf,
}

impl GameDir {
    pub fn new(root_path: std::path::PathBuf) -> Self {
        GameDir { root_path }
    }

    pub fn main_resource_file(&self) -> data_block::Result<ResourceFile> {
        let resource_map =
            PathBlockSource::new(self.root_path.join("resource.map")).into_source_box();
        let resource_data =
            PathBlockSource::new(self.root_path.join("resource.data")).into_source_box();
        Ok(ResourceFile::new(resource_map, resource_data))
    }
}
