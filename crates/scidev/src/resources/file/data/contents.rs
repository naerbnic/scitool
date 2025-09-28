use std::io;

use crate::{
    resources::{
        ConversionError, ResourceId, ResourceType, file::data::raw_header::RawEntryHeader,
    },
    utils::{
        block::{BlockSource, LazyBlock},
        compression::dcl::decompress_dcl,
        errors::{OtherError, prelude::*},
    },
};

struct RawContents<'a> {
    header: RawEntryHeader,
    data: BlockSource<'a>,
}

impl std::fmt::Debug for RawContents<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RawContents")
            .field("header", &self.header)
            .field("data", &self.data.size())
            .finish()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Contents<'a> {
    id: ResourceId,
    data: LazyBlock<'a>,
}

impl<'a> Contents<'a> {
    pub(super) fn from_parts(
        header: RawEntryHeader,
        resource_block: BlockSource<'a>,
    ) -> Result<Self, ConversionError> {
        assert_eq!(resource_block.size(), u64::from(header.packed_size()));
        let raw_contents = RawContents {
            header,
            data: resource_block,
        };

        let decompressed_data = match raw_contents.header.compression_type() {
            0 => raw_contents.data.to_lazy_block(),
            18 => raw_contents
                .data
                .to_lazy_block()
                .map(move |block| Ok(decompress_dcl(&block).with_other_err()?)),
            _ => {
                // Let's be lazy here.
                LazyBlock::from_factory(move || {
                    Err(OtherError::from_msg(format!(
                        "Unsupported compression type: {}",
                        raw_contents.header.compression_type()
                    ))
                    .into())
                })
            }
        };
        let decompressed_data = decompressed_data.with_check(move |block| {
            if block.size() != raw_contents.header.unpacked_size() as usize {
                return Err(io::Error::other("Decompressed data size mismatch").into());
            }
            Ok(())
        });

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
    pub(crate) fn data(&self) -> &LazyBlock<'a> {
        &self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use datalit::datalit;

    use crate::utils::{mem_reader::Parse, testing::block::mem_reader_from_bytes};

    #[tokio::test]
    async fn test_basic_contents() {
        let header_data = datalit! {
            @endian = le,
            0x80u8,
            100u16,
            4u16,
            4u16,
            0u16,
        };
        let header = RawEntryHeader::parse(&mut mem_reader_from_bytes(header_data)).unwrap();
        let content_source = BlockSource::from_vec(
            datalit! {
                0x00010203
            }
            .to_vec(),
        );
        let contents = Contents::from_parts(header, content_source).unwrap();

        let content_data = contents.data().open().await.unwrap();
        assert_eq!(content_data.as_ref(), &[0, 1, 2, 3]);
    }
}
