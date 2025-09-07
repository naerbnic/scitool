use crate::utils::mem_reader::{self, MemReader};

#[derive(Debug)]
pub(super) struct ResourceLocationEntry {
    resource_num: u16,
    resource_file_offset: u32,
}

impl ResourceLocationEntry {
    pub(super) fn resource_num(&self) -> u16 {
        self.resource_num
    }

    pub(super) fn resource_file_offset(&self) -> u32 {
        self.resource_file_offset
    }
}

impl ResourceLocationEntry {
    pub(crate) fn read_from<M: MemReader>(
        reader: &mut M,
    ) -> mem_reader::Result<ResourceLocationEntry, M::Error> {
        let resource_num = reader.read_u16_le()?;
        let body = reader.read_u24_le()?;
        assert_eq!(body & 0xF000_0000, 0);
        let resource_file_offset = (body & 0x0FFF_FFFF) << 1;
        Ok(ResourceLocationEntry {
            resource_num,
            resource_file_offset,
        })
    }
}
