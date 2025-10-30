use std::io;

use crate::{
    resources::{
        ConversionError, file::volume::raw_contents::RawContents, resource::CompressedData,
    },
    utils::{block::Block, compression::dcl::DecompressFactory},
};

#[derive(Debug, Clone)]
pub(crate) struct Contents {
    compressed: Option<CompressedData>,
    data: Block,
}

impl Contents {
    pub(super) fn from_raw(raw_contents: &RawContents) -> Result<Self, ConversionError> {
        let raw_data = raw_contents.data().clone();
        let (compressed, data) = match raw_contents.compression_type() {
            0 => (None, raw_data),
            compression_type @ 18..=20 => {
                let unpacked_size = u64::from(raw_contents.unpacked_size());

                let decompressed_data = Block::builder()
                    .with_size(unpacked_size)
                    .build_from_read_factory(DecompressFactory::new(raw_data.clone()))
                    .map_err(ConversionError::new)?;
                (
                    Some(CompressedData::new(compression_type, raw_data)),
                    decompressed_data,
                )
            }
            compression_type => {
                // Let's be lazy here.
                (
                    Some(CompressedData::new(compression_type, raw_data)),
                    Block::from_error_fn(move || {
                        io::Error::other(format!(
                            "Unsupported compression type: {compression_type}"
                        ))
                    }),
                )
            }
        };

        Ok(Contents { compressed, data })
    }

    pub(crate) fn compressed(&self) -> Option<&CompressedData> {
        self.compressed.as_ref()
    }

    pub(crate) fn data(&self) -> &Block {
        &self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use datalit::datalit;

    use crate::{
        resources::file::volume::raw_header::RawEntryHeader,
        utils::{mem_reader::Parse, testing::block::mem_reader_from_bytes},
    };

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
        let raw_contents = RawContents::new(0, header, content_source);
        let contents = Contents::from_raw(&raw_contents).unwrap();

        let content_data = contents.data().open_mem(..).unwrap();
        assert_eq!(content_data.as_ref(), &[0, 1, 2, 3]);
    }
}
