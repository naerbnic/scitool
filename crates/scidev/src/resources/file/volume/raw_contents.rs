use crate::{
    resources::{ResourceId, ResourceType, file::volume::raw_header::RawEntryHeader},
    utils::block::Block,
};

pub(crate) struct RawContents {
    header_offset: u32,
    id: ResourceId,
    unpacked_size: u16,
    compression_type: u16,
    data: Block,
}

impl RawContents {
    pub(crate) fn new(header_offset: u32, header: RawEntryHeader, data: Block) -> Self {
        Self {
            header_offset,
            id: ResourceId::new(
                ResourceType::try_from(header.res_type()).unwrap(),
                header.res_number(),
            ),
            unpacked_size: header.unpacked_size(),
            compression_type: header.compression_type(),
            data,
        }
    }

    #[expect(dead_code, reason = "Will be used in the near future")]
    pub(crate) fn header_offset(&self) -> u32 {
        self.header_offset
    }

    pub(crate) fn id(&self) -> ResourceId {
        self.id
    }

    pub(crate) fn compression_type(&self) -> u16 {
        self.compression_type
    }

    pub(crate) fn unpacked_size(&self) -> u16 {
        self.unpacked_size
    }

    pub(crate) fn data(&self) -> &Block {
        &self.data
    }
}
