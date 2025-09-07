use std::io;

use crate::{
    resources::{
        ConversionError, ResourceId, ResourceType, file::data::raw_header::RawEntryHeader,
    },
    utils::{
        block::{BlockSource, LazyBlock},
        compression::dcl::decompress_dcl,
        errors::prelude::*,
    },
};

struct RawContents {
    header: RawEntryHeader,
    data: BlockSource,
}

impl std::fmt::Debug for RawContents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RawContents")
            .field("header", &self.header)
            .field("data", &self.data.size())
            .finish()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Contents {
    id: ResourceId,
    data: LazyBlock,
}

impl Contents {
    pub(super) fn from_parts(
        header: RawEntryHeader,
        resource_block: BlockSource,
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
                    Err(io::Error::other(format!(
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
    pub(crate) fn data(&self) -> &LazyBlock {
        &self.data
    }
}
