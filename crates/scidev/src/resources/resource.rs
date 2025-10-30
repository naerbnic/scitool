use std::io;

use crate::{
    resources::{
        ExtraData, ResourceId,
        file::{
            CompressedData, ResourceContents, ResourcePatchError, write_resource_to_patch_file,
        },
    },
    utils::block::Block,
};

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
