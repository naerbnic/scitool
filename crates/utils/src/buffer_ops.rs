pub trait BufferOpsExt {
    fn read_u16_le_at(&self, offset: usize) -> u16;
}

impl BufferOpsExt for [u8] {
    fn read_u16_le_at(&self, offset: usize) -> u16 {
        let bytes = &self[offset..][..2];
        u16::from_le_bytes(bytes.try_into().unwrap())
    }
}
