use std::io;

use crate::{
    resources::{
        ResourceId,
        file::{ResourcePatchError, write_resource_to_patch_file},
    },
    utils::block::Block,
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
pub(crate) struct ResourceContents {
    /// Any extra data associated with the resource.
    ///
    /// This is typically only present if the resource was loaded from a
    /// patch file.
    extra_data: Option<ExtraData>,

    /// If the block was originally compressed, this contains the compressed
    /// data and necessary metadata.
    compressed: Option<CompressedData>,

    /// The main data source for the resource.
    source: Block,
}

impl ResourceContents {
    #[must_use]
    pub(crate) fn from_source(source: Block) -> Self {
        ResourceContents {
            extra_data: None,
            compressed: None,
            source,
        }
    }

    #[must_use]
    pub(crate) fn from_extra_data_source(extra_data: ExtraData, source: Block) -> Self {
        ResourceContents {
            extra_data: Some(extra_data),
            compressed: None,
            source,
        }
    }

    #[must_use]
    pub(crate) fn from_compressed_source(compressed: CompressedData, source: Block) -> Self {
        ResourceContents {
            extra_data: None,
            compressed: Some(compressed),
            source,
        }
    }

    pub(crate) fn extra_data(&self) -> Option<&ExtraData> {
        self.extra_data.as_ref()
    }

    pub(crate) fn compressed(&self) -> Option<&CompressedData> {
        self.compressed.as_ref()
    }

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
    pub(crate) fn from_contents(id: ResourceId, contents: ResourceContents) -> Resource {
        Resource { id, contents }
    }

    #[must_use]
    pub(crate) fn contents(&self) -> &ResourceContents {
        &self.contents
    }

    #[must_use]
    pub fn id(&self) -> &ResourceId {
        &self.id
    }

    #[must_use]
    pub fn extra_data(&self) -> Option<&ExtraData> {
        self.contents.extra_data()
    }

    #[must_use]
    pub fn compressed(&self) -> Option<&CompressedData> {
        self.contents.compressed()
    }

    #[must_use]
    pub fn data(&self) -> &Block {
        self.contents.source()
    }

    pub fn write_patch<W: io::Write>(&self, writer: W) -> Result<(), ResourcePatchError> {
        write_resource_to_patch_file(self, writer)
    }
}
