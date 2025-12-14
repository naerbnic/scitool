use std::io;

use crate::{
    resources::{ResourceId, file::write_resource_to_patch_file},
    utils::{block::Block, errors::OpaqueError},
};

#[derive(Debug, Clone)]
pub enum ExtraData {
    Simple(Block),
    Composite {
        ext_header: Block,
        extra_data: Block,
    },
}

#[derive(Debug, Clone)]
pub struct CompressedData {
    compression_type: u16,
    compressed_block: Block,
}

impl CompressedData {
    pub fn new(compression_type: u16, compressed_block: Block) -> Self {
        CompressedData {
            compression_type,
            compressed_block,
        }
    }
    #[must_use]
    pub fn compression_type(&self) -> u16 {
        self.compression_type
    }

    #[must_use]
    pub fn compressed_block(&self) -> &Block {
        &self.compressed_block
    }
}

#[derive(Debug, Clone)]
pub struct VolumeSource {
    archive_num: u16,
    archive_offset: u32,
    compressed_data: Option<CompressedData>,
}

impl VolumeSource {
    #[must_use]
    pub(super) fn new(archive_offset: u32, compressed_data: Option<CompressedData>) -> Self {
        VolumeSource {
            archive_num: 0,
            archive_offset,
            compressed_data,
        }
    }

    #[must_use]
    pub fn archive_num(&self) -> u16 {
        self.archive_num
    }

    #[must_use]
    pub fn archive_offset(&self) -> u32 {
        self.archive_offset
    }

    #[must_use]
    pub fn compressed_data(&self) -> Option<&CompressedData> {
        self.compressed_data.as_ref()
    }
}

#[derive(Debug, Clone)]
pub struct PatchSource {
    patch_header_data: ExtraData,
}

impl PatchSource {
    #[must_use]
    pub fn new(patch_header_data: ExtraData) -> Self {
        PatchSource { patch_header_data }
    }

    #[must_use]
    pub fn extra_data(&self) -> &ExtraData {
        &self.patch_header_data
    }
}

#[derive(Debug, Clone)]
pub enum ResourceProvenance {
    Volume(VolumeSource),
    PatchFile(PatchSource),
    New,
}

#[derive(Debug, Clone)]
pub(crate) struct ResourceContents {
    provenance: ResourceProvenance,

    /// The main data source for the resource.
    source: Block,
}

impl ResourceContents {
    #[must_use]
    pub(super) fn from_volume(source: VolumeSource, data: Block) -> Self {
        ResourceContents {
            provenance: ResourceProvenance::Volume(source),
            source: data,
        }
    }

    #[must_use]
    pub(super) fn from_patch(patch: PatchSource, data: Block) -> Self {
        ResourceContents {
            provenance: ResourceProvenance::PatchFile(patch),
            source: data,
        }
    }

    #[must_use]
    pub(crate) fn new(source: Block) -> Self {
        ResourceContents {
            provenance: ResourceProvenance::New,
            source,
        }
    }

    pub(crate) fn extra_data(&self) -> Option<&ExtraData> {
        match &self.provenance {
            ResourceProvenance::PatchFile(patch) => Some(&patch.patch_header_data),
            _ => None,
        }
    }

    #[must_use]
    pub(crate) fn compressed(&self) -> Option<&CompressedData> {
        match &self.provenance {
            ResourceProvenance::Volume(volume) => volume.compressed_data.as_ref(),
            _ => None,
        }
    }

    #[must_use]
    pub(crate) fn provenance(&self) -> &ResourceProvenance {
        &self.provenance
    }

    #[must_use]
    pub(crate) fn source(&self) -> &Block {
        &self.source
    }
}

#[derive(Debug, Clone)]
pub struct Resource {
    /// The ID of the resource.
    id: ResourceId,
    contents: ResourceContents,
}

impl Resource {
    #[must_use]
    pub(crate) fn new(id: ResourceId, contents: ResourceContents) -> Self {
        Resource { id, contents }
    }

    #[must_use]
    pub(crate) fn contents(&self) -> &ResourceContents {
        &self.contents
    }

    #[must_use]
    pub fn provenance(&self) -> &ResourceProvenance {
        self.contents.provenance()
    }

    #[must_use]
    pub fn compressed(&self) -> Option<&CompressedData> {
        self.contents.compressed()
    }

    #[must_use]
    pub fn extra_data(&self) -> Option<&ExtraData> {
        self.contents.extra_data()
    }

    #[must_use]
    pub fn id(&self) -> &ResourceId {
        &self.id
    }

    #[must_use]
    pub fn data(&self) -> &Block {
        self.contents.source()
    }

    pub fn write_patch<W: io::Write>(&self, writer: W) -> Result<(), OpaqueError> {
        write_resource_to_patch_file(self, writer)
    }
}
