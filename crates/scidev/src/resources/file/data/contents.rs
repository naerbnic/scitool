use std::io;

use crate::{
    resources::{
        ConversionError, ResourceId, ResourceType, file::data::raw_header::RawEntryHeader,
    },
    utils::{block::Block, compression::dcl::decompress_dcl},
};

struct RawContents {
    header: RawEntryHeader,
    data: Block,
}

impl std::fmt::Debug for RawContents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RawContents")
            .field("header", &self.header)
            .field("data", &self.data.len())
            .finish()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Contents {
    id: ResourceId,
    data: Block,
}

impl Contents {
    pub(super) fn from_parts(
        header: RawEntryHeader,
        resource_block: Block,
    ) -> Result<Self, ConversionError> {
        assert_eq!(resource_block.len(), u64::from(header.packed_size()));
        let raw_contents = RawContents {
            header,
            data: resource_block,
        };

        let decompressed_data = match raw_contents.header.compression_type() {
            0 => raw_contents.data,
            18..=20 => {
                let unpacked_size = raw_contents.header.unpacked_size() as u64;
                let data = raw_contents.data.clone();
                Block::builder()
                    .with_size(unpacked_size)
                    .build_from_mem_block_factory(move || {
                        decompress_dcl(&data.open_mem(..)?).map_err(io::Error::other)
                    })
                    .map_err(ConversionError::new)?
            }
            _ => {
                // Let's be lazy here.
                Block::from_error_fn(move || {
                    io::Error::other(format!(
                        "Unsupported compression type: {}",
                        raw_contents.header.compression_type()
                    ))
                })
            }
        };

        Ok(Contents {
            id: ResourceId::new(
                ResourceType::try_from(raw_contents.header.res_type())?,
                raw_contents.header.res_number(),
            ),
            data: decompressed_data,
        })
    }

    pub(crate) fn id(&self) -> &ResourceId {
        &self.id
    }

    pub(crate) fn data(&self) -> &Block {
        &self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use datalit::datalit;

    use crate::utils::{mem_reader::Parse, testing::block::mem_reader_from_bytes};

    #[test]
    fn test_basic_contents() {
        let header_data = datalit! {
            @endian = le,
            0x80u8,
            100u16,
            4u16,
            4u16,
            0u16,
        };
        let header = RawEntryHeader::parse(&mut mem_reader_from_bytes(header_data)).unwrap();
        let content_source = Block::from_vec(
            datalit! {
                0x00010203
            }
            .to_vec(),
        );
        let contents = Contents::from_parts(header, content_source).unwrap();

        let content_data = contents.data().open_mem(..).unwrap();
        assert_eq!(content_data.as_ref(), &[0, 1, 2, 3]);
    }
}
