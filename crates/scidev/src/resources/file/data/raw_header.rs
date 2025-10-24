use crate::utils::{
    block::FromBlock,
    mem_reader::{self, MemReader, Parse},
};

/// A resource entry header in a data file.
///
/// This is based on the SCI1.1 data file format.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RawEntryHeader {
    res_type: u8,
    res_number: u16,
    packed_size: u16,
    unpacked_size: u16,
    compression_type: u16,
}

impl RawEntryHeader {
    pub(crate) fn res_type(&self) -> u8 {
        self.res_type & 0x7F
    }

    pub(crate) fn res_number(&self) -> u16 {
        self.res_number
    }

    pub(crate) fn packed_size(&self) -> u16 {
        self.packed_size
    }

    pub(crate) fn unpacked_size(&self) -> u16 {
        self.unpacked_size
    }

    pub(crate) fn compression_type(&self) -> u16 {
        self.compression_type
    }
}

impl Parse for RawEntryHeader {
    fn parse<M: MemReader>(reader: &mut M) -> mem_reader::Result<Self, M::Error> {
        let res_type = reader.read_u8()?;
        let res_number = reader.read_u16_le()?;
        let packed_size = reader.read_u16_le()?;
        let unpacked_size = reader.read_u16_le()?;
        let compression_type = reader.read_u16_le()?;
        Ok(RawEntryHeader {
            res_type,
            res_number,
            packed_size,
            unpacked_size,
            compression_type,
        })
    }
}

impl FromBlock for RawEntryHeader {
    fn read_size() -> usize {
        9
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::testing::block::mem_reader_from_bytes;

    use super::*;
    use datalit::datalit;

    #[test]
    fn test_read_raw_entry_header() {
        let data = datalit! {
            @endian = le,
            1u8,                // res_type
            42u16,              // res_number
            10u16,              // packed_size
            20u16,              // unpacked_size
            0u16,               // compression_type
        };

        let mut reader = mem_reader_from_bytes(data);
        let header: RawEntryHeader = mem_reader::Parse::parse(&mut reader).unwrap();
        assert_eq!(reader.tell(), 9);
        assert_eq!(header.res_type, 1);
        assert_eq!(header.res_number, 42);
        assert_eq!(header.packed_size, 10);
        assert_eq!(header.unpacked_size, 20);
        assert_eq!(header.compression_type, 0);
    }
}
