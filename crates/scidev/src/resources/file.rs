use std::{
    collections::{BTreeMap, btree_map},
    error::Error as StdError,
    fs::File,
    io::{self, Write},
    path::Path,
};

use data::DataFile;

use self::patch::try_patch_from_file;
use crate::{
    resources::{ConversionError, file::patch::write_resource_to_patch_file},
    utils::{
        block::{BlockSource, BlockSourceError, LazyBlock, MemBlock, MemBlockFromReaderError},
        errors::{AnyInvalidDataError, NoError, OtherError, prelude::*},
        mem_reader::{self, BufferMemReader, Parse as _},
    },
};

use super::{ResourceId, ResourceType};

pub(super) use self::patch::ResourcePatchError;

mod data;
mod map;
mod patch;

#[derive(Debug, thiserror::Error)]
pub(super) enum Error {
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
    #[error(transparent)]
    Other(Box<dyn StdError + Send + Sync>),
}

impl From<data::Error> for Error {
    fn from(err: data::Error) -> Self {
        match err {
            data::Error::Io(io_err) => Self::Io(io_err),
            data::Error::MemReader(mem_err) => Self::MalformedData(mem_err),
            data::Error::Conversion(err) => Self::Conversion(err),
            e @ data::Error::InvalidResourceLocation { .. } => Self::Other(Box::new(e)),
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

pub(super) fn read_resources(
    map_file: &Path,
    data_file: &Path,
    patches: &[Resource],
) -> Result<ResourceSet, Error> {
    let map_file = MemBlock::from_reader(File::open(map_file)?)?;
    let data_file = DataFile::new(BlockSource::from_path(data_file.to_path_buf())?);
    let resource_locations =
        map::ResourceLocationSet::parse(&mut BufferMemReader::from_ref(&map_file))?;

    let mut entries = BTreeMap::new();

    for location in resource_locations.locations() {
        let block = data_file.read_contents(location)?;
        if block.id() != &location.id() {
            return Err(Error::ResourceIdMismatch {
                expected: location.id(),
                got: *block.id(),
            });
        }
        entries.insert(
            location.id(),
            ResourceBlocks::new_of_data(ResourceContents::from_source(block.data().clone())),
        );
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
pub enum OpenGameResourcesError {
    #[doc(hidden)]
    #[error(transparent)]
    Other(#[from] OtherError),
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
            .map(|b| Resource::from_contents(*id, b.default_contents().clone()))
    }

    pub fn resource_ids(&self) -> impl Iterator<Item = ResourceId> + '_ {
        self.entries.keys().copied()
    }

    pub fn resources(&self) -> impl Iterator<Item = Resource> + '_ {
        self.entries
            .iter()
            .map(|(id, block)| Resource::from_contents(*id, block.default_contents().clone()))
    }

    pub fn resources_of_type(&self, type_id: ResourceType) -> impl Iterator<Item = Resource> + '_ {
        self.entries.iter().filter_map(move |(id, block)| {
            if id.type_id != type_id {
                return None;
            }
            Some(Resource::from_contents(
                *id,
                block.default_contents().clone(),
            ))
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

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct ResourceLoadError(#[from] OtherError);

#[derive(Clone)]
pub enum ExtraData {
    Simple(LazyBlock),
    Composite {
        ext_header: LazyBlock,
        extra_data: LazyBlock,
    },
}

#[derive(Clone)]
struct ResourceContents {
    /// Any extra data associated with the resource.
    ///
    /// This is typically only present if the resource was loaded from a
    /// patch file.
    extra_data: Option<ExtraData>,

    /// The main data source for the resource.
    source: LazyBlock,
}

impl ResourceContents {
    #[must_use]
    pub(crate) fn from_source(source: LazyBlock) -> Self {
        ResourceContents {
            extra_data: None,
            source,
        }
    }
}

pub struct Resource {
    /// The ID of the resource.
    id: ResourceId,

    contents: ResourceContents,
}

impl Resource {
    #[must_use]
    pub fn new(id: ResourceId, source: LazyBlock) -> Self {
        Resource {
            id,
            contents: ResourceContents {
                extra_data: None,
                source,
            },
        }
    }

    #[must_use]
    fn from_contents(id: ResourceId, contents: ResourceContents) -> Resource {
        Resource { id, contents }
    }

    #[must_use]
    pub fn id(&self) -> &ResourceId {
        &self.id
    }

    #[must_use]
    pub fn extra_data(&self) -> Option<&ExtraData> {
        self.contents.extra_data.as_ref()
    }

    #[must_use]
    fn contents(&self) -> &ResourceContents {
        &self.contents
    }

    pub fn load_data(&self) -> Result<MemBlock, ResourceLoadError> {
        Ok(self.contents.source.open().with_other_err()?)
    }

    pub fn write_patch<W: Write>(&self, writer: W) -> Result<(), ResourcePatchError> {
        write_resource_to_patch_file(self, writer)
    }
}
