use std::{
    collections::{BTreeMap, btree_map},
    fs::File,
    io,
    path::Path,
};

use data::DataFile;

use self::patch::try_patch_from_file;
use crate::{
    resources::ConversionError,
    utils::{
        block::{BlockSource, BlockSourceError, LazyBlock, MemBlock, MemBlockFromReaderError},
        errors::{AnyInvalidDataError, NoError, OtherError, prelude::*},
        mem_reader::{self, BufferMemReader},
    },
};

use super::{ResourceId, ResourceType};

use tokio::io::AsyncWriteExt;

mod data;
mod map;
mod patch;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error during operation: {0}")]
    Io(#[from] io::Error),
    #[error("Malformed data: {0}")]
    MalformedData(#[from] AnyInvalidDataError),
    #[error(transparent)]
    Conversion(#[from] ConversionError),
    #[error("Resource ID mismatch: expected {expected:?}, got {got:?}")]
    ResourceIdMismatch {
        expected: ResourceId,
        got: ResourceId,
    },
}

impl From<data::Error> for Error {
    fn from(err: data::Error) -> Self {
        match err {
            data::Error::Io(io_err) => Self::Io(io_err),
            data::Error::MemReader(mem_err) => Self::MalformedData(mem_err),
            data::Error::Conversion(err) => Self::Conversion(err),
        }
    }
}

impl From<mem_reader::Error<NoError>> for Error {
    fn from(err: mem_reader::Error<NoError>) -> Self {
        match err {
            mem_reader::Error::InvalidData(invalid_data_err) => {
                Self::MalformedData(invalid_data_err)
            }
            mem_reader::Error::BaseError(err) => err.absurd(),
        }
    }
}

impl From<BlockSourceError> for Error {
    fn from(err: BlockSourceError) -> Self {
        match err {
            BlockSourceError::Io(io_err) => Self::Io(io_err),
            BlockSourceError::Conversion(conv_err) => {
                Self::Conversion(ConversionError::new(conv_err))
            }
        }
    }
}

impl From<MemBlockFromReaderError> for Error {
    fn from(err: MemBlockFromReaderError) -> Self {
        match err {
            MemBlockFromReaderError::Io(io_err) => Self::Io(io_err),
            MemBlockFromReaderError::Conversion(conv_err) => {
                Self::Conversion(ConversionError::new(conv_err))
            }
        }
    }
}

pub fn read_resources(
    map_file: &Path,
    data_file: &Path,
    patches: &[Resource],
) -> Result<ResourceSet, Error> {
    let map_file = MemBlock::from_reader(File::open(map_file)?)?;
    let data_file = DataFile::new(BlockSource::from_path(data_file.to_path_buf())?);
    let resource_locations =
        map::ResourceLocations::read_from(&mut BufferMemReader::from_ref(&map_file))?;

    let mut entries = BTreeMap::new();

    for location in resource_locations.locations() {
        let block = data_file.read_contents(location)?;
        if block.id() != &location.id {
            return Err(Error::ResourceIdMismatch {
                expected: location.id,
                got: *block.id(),
            });
        }
        entries.insert(
            location.id,
            ResourceBlocks::new_of_data(block.data().clone()),
        );
    }

    for patch in patches {
        let id = patch.id();
        match entries.entry(*id) {
            btree_map::Entry::Vacant(vac) => {
                vac.insert(ResourceBlocks::new_of_patch(patch.source.clone()));
            }
            btree_map::Entry::Occupied(occ) => occ.into_mut().add_patch(patch.source.clone()),
        }
    }

    Ok(ResourceSet { entries })
}

#[derive(Clone)]
struct ResourceBlocks {
    data_block: Option<LazyBlock>,
    patch_block: Option<LazyBlock>,
}

impl ResourceBlocks {
    pub(crate) fn default_block(&self) -> &LazyBlock {
        self.patch_block
            .as_ref()
            .or(self.data_block.as_ref())
            .expect("Resource block not found")
    }

    pub(crate) fn new_of_patch(patch_block: LazyBlock) -> ResourceBlocks {
        ResourceBlocks {
            data_block: None,
            patch_block: Some(patch_block),
        }
    }

    pub(crate) fn new_of_data(data_block: LazyBlock) -> ResourceBlocks {
        ResourceBlocks {
            data_block: Some(data_block),
            patch_block: None,
        }
    }

    pub(crate) fn add_patch(&mut self, patch_block: LazyBlock) {
        if self.patch_block.is_none() {
            self.patch_block = Some(patch_block);
        } else {
            panic!("Resource already has a patch block");
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum OpenGameResourcesError {
    #[doc(hidden)]
    #[error(transparent)]
    Other(#[from] OtherError),
}

pub struct ResourceSet {
    entries: BTreeMap<ResourceId, ResourceBlocks>,
}

impl ResourceSet {
    #[must_use]
    pub fn get_resource(&self, id: &ResourceId) -> Option<Resource> {
        self.entries.get(id).map(|b| Resource {
            id: *id,
            source: b.default_block().clone(),
        })
    }

    pub fn resource_ids(&self) -> impl Iterator<Item = ResourceId> + '_ {
        self.entries.keys().copied()
    }

    pub fn resources(&self) -> impl Iterator<Item = Resource> + '_ {
        self.entries.iter().map(|(id, block)| Resource {
            id: *id,
            source: block.default_block().clone(),
        })
    }

    pub fn resources_of_type(&self, type_id: ResourceType) -> impl Iterator<Item = Resource> + '_ {
        self.entries.iter().filter_map(move |(id, block)| {
            if id.type_id != type_id {
                return None;
            }
            Some(Resource {
                id: *id,
                source: block.default_block().clone(),
            })
        })
    }

    #[must_use]
    pub fn with_overlay(&self, overlay: &ResourceSet) -> ResourceSet {
        let mut entries = self.entries.clone();
        for (id, block) in &overlay.entries {
            entries.insert(*id, block.clone());
        }
        ResourceSet { entries }
    }

    pub fn merge(&self, other: &ResourceSet) -> io::Result<ResourceSet> {
        let mut entries = self.entries.clone();
        for (id, block) in &other.entries {
            match entries.entry(*id) {
                btree_map::Entry::Vacant(vac) => {
                    vac.insert(block.clone());
                }
                btree_map::Entry::Occupied(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Duplicate resource ID: {id:?}"),
                    ));
                }
            }
        }
        Ok(ResourceSet { entries })
    }
}

pub fn open_game_resources(root_dir: &Path) -> Result<ResourceSet, OpenGameResourcesError> {
    let mut patches = Vec::new();
    for entry in root_dir.read_dir().with_other_err()? {
        let entry = entry.with_other_err()?;
        if entry.file_type().with_other_err()?.is_file()
            && let Some(patch_res) = try_patch_from_file(&entry.path()).with_other_err()?
        {
            patches.push(patch_res);
        }
    }

    let main_set = {
        let map_file = root_dir.join("RESOURCE.MAP");
        let data_file = root_dir.join("RESOURCE.000");
        read_resources(&map_file, &data_file, &patches).with_other_err()?
    };

    let message_set = {
        let map_file = root_dir.join("MESSAGE.MAP");
        let data_file = root_dir.join("RESOURCE.MSG");
        read_resources(&map_file, &data_file, &[]).with_other_err()?
    };
    Ok(main_set.merge(&message_set).with_other_err()?)
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct ResourceLoadError(#[from] OtherError);

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct ResourcePatchError(#[from] OtherError);

pub struct Resource {
    id: ResourceId,
    source: LazyBlock,
}

impl Resource {
    #[must_use]
    pub fn new(id: ResourceId, source: LazyBlock) -> Resource {
        Resource { id, source }
    }
}

impl Resource {
    #[must_use]
    pub fn id(&self) -> &ResourceId {
        &self.id
    }

    pub fn load_data(&self) -> Result<MemBlock, ResourceLoadError> {
        Ok(self.source.open().with_other_err()?)
    }

    pub async fn write_patch<W: tokio::io::AsyncWrite + Unpin>(
        &self,
        mut writer: W,
    ) -> Result<(), ResourcePatchError> {
        writer
            .write_all(&[self.id.type_id().into(), 0])
            .await
            .with_other_err()?;
        let data = self.source.open().with_other_err()?;
        writer.write_all(&data).await.with_other_err()?;
        Ok(())
    }
}
