use std::{
    collections::{BTreeMap, btree_map},
    error::Error as StdError,
    fs::File,
    io::{self},
    path::Path,
};

use volume::VolumeFile;

use self::patch::try_patch_from_file;
use crate::{
    resources::{
        ConversionError,
        file::map::MapFile,
        resource::{Resource, ResourceContents, VolumeSource},
    },
    utils::{
        block::{Block, MemBlockFromReaderError},
        errors::{
            AnyInvalidDataError, BoxError, DynError, ErrWrapper, NoError, OtherError, prelude::*,
        },
        mem_reader,
    },
};

use super::{ResourceId, ResourceType};

pub(super) use self::patch::{ResourcePatchError, write_resource_to_patch_file};

mod map;
mod patch;
mod volume;

#[derive(Debug, thiserror::Error)]
pub(super) enum Error {
    #[error("I/O error during operation: {0}")]
    Io(io::Error),
    #[error("Malformed data: {0}")]
    MalformedData(#[from] AnyInvalidDataError),
    #[error(transparent)]
    Conversion(#[from] ConversionError),
    #[error("Resource ID mismatch: expected {expected:?}, got {got:?}")]
    ResourceIdMismatch {
        expected: ResourceId,
        got: ResourceId,
    },
    #[error(transparent)]
    Other(Box<dyn StdError + Send + Sync>),
}

impl From<volume::Error> for Error {
    fn from(err: volume::Error) -> Self {
        match err {
            volume::Error::Io(io_err) => Self::Io(io_err),
            volume::Error::MemReader(mem_err) => Self::MalformedData(mem_err),
            volume::Error::Conversion(err) => Self::Conversion(err),
            e @ volume::Error::InvalidResourceLocation { .. } => Self::Other(Box::new(e)),
            volume::Error::ResourceIdMismatch { expected, got } => {
                Self::ResourceIdMismatch { expected, got }
            }
        }
    }
}

impl From<mem_reader::MemReaderError<NoError>> for Error {
    fn from(err: mem_reader::MemReaderError<NoError>) -> Self {
        match err {
            mem_reader::MemReaderError::InvalidData(invalid_data_err) => {
                Self::MalformedData(invalid_data_err)
            }
            mem_reader::MemReaderError::Base(err) => err.absurd(),
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        match err.downcast() {
            Ok(err) => err,
            Err(err) => Self::Io(err),
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

pub(super) fn read_resources(
    map_file: &Path,
    data_file: &Path,
    patches: &[Resource],
) -> Result<ResourceSet, Error> {
    let map_file = MapFile::from_read_seek(File::open(map_file)?)?;
    let data_file = VolumeFile::new(Block::from_path(data_file.to_path_buf())?);

    let mut entries = BTreeMap::new();

    for location in map_file.locations() {
        let block = data_file.read_contents(location)?;
        let volume_source = VolumeSource::new(location.file_offset(), block.compressed().cloned());
        let contents = ResourceContents::from_volume(volume_source, block.data().clone());
        entries.insert(location.id(), ResourceBlocks::new_of_data(contents));
    }

    for patch in patches {
        let id = patch.id();
        match entries.entry(*id) {
            btree_map::Entry::Vacant(vac) => {
                vac.insert(ResourceBlocks::new_of_patch(patch.contents().clone()));
            }
            btree_map::Entry::Occupied(occ) => occ.into_mut().add_patch(patch.contents().clone()),
        }
    }

    Ok(ResourceSet { entries })
}

#[derive(Clone)]
struct ResourceBlocks {
    data_contents: Option<ResourceContents>,
    patch_contents: Option<ResourceContents>,
}

impl ResourceBlocks {
    pub(crate) fn default_contents(&self) -> &ResourceContents {
        self.data_contents
            .as_ref()
            .or(self.patch_contents.as_ref())
            .expect("Resource block not found")
    }

    pub(crate) fn new_of_patch(patch_contents: ResourceContents) -> ResourceBlocks {
        ResourceBlocks {
            data_contents: None,
            patch_contents: Some(patch_contents),
        }
    }

    pub(crate) fn new_of_data(data_contents: ResourceContents) -> ResourceBlocks {
        ResourceBlocks {
            data_contents: Some(data_contents),
            patch_contents: None,
        }
    }

    pub(crate) fn add_patch(&mut self, patch_contents: ResourceContents) {
        if self.patch_contents.is_none() {
            self.patch_contents = Some(patch_contents);
        } else {
            panic!("Resource already has a patch block");
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct OpenGameResourcesError(#[from] OtherError);

impl ErrWrapper for OpenGameResourcesError {
    fn wrapped_err(&self) -> Option<&DynError> {
        self.0.wrapped_err()
    }

    fn try_unwrap_box(self) -> Result<BoxError, Self> {
        match self.0.try_unwrap_box() {
            Ok(boxed) => Ok(boxed),
            Err(wrap) => Err(OpenGameResourcesError(wrap)),
        }
    }

    fn wrap_box(err: BoxError) -> Self {
        OpenGameResourcesError(OtherError::wrap_box(err))
    }
}

pub struct ResourceSet {
    entries: BTreeMap<ResourceId, ResourceBlocks>,
}

impl ResourceSet {
    pub fn from_root_dir(root_dir: &Path) -> Result<Self, OpenGameResourcesError> {
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
    #[must_use]
    pub fn get_resource(&self, id: &ResourceId) -> Option<Resource> {
        self.entries
            .get(id)
            .map(|b| Resource::new(*id, b.default_contents().clone()))
    }

    pub fn resource_ids(&self) -> impl Iterator<Item = ResourceId> + '_ {
        self.entries.keys().copied()
    }

    pub fn resources(&self) -> impl Iterator<Item = Resource> + '_ {
        self.entries
            .iter()
            .map(|(id, block)| Resource::new(*id, block.default_contents().clone()))
    }

    pub fn resources_of_type(&self, type_id: ResourceType) -> impl Iterator<Item = Resource> + '_ {
        self.entries.iter().filter_map(move |(id, block)| {
            if id.type_id != type_id {
                return None;
            }
            Some(Resource::new(*id, block.default_contents().clone()))
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
