use crate::resources::ResourceId;

/// The location of a resource within a resource data file
#[derive(Debug, Clone, Copy)]
pub(crate) struct ResourceLocation {
    id: ResourceId,
    file_offset: u32,
}

impl ResourceLocation {
    pub(crate) fn new(id: ResourceId, file_offset: u32) -> Self {
        ResourceLocation { id, file_offset }
    }

    pub(crate) fn id(self) -> ResourceId {
        self.id
    }

    pub(crate) fn file_offset(self) -> u32 {
        self.file_offset
    }
}
