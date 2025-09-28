use crate::{
    resources::file::map::ResourceLocation,
    utils::block::{BlockSource, FromBlockSource},
};

use super::{contents::Contents, errors::Error, raw_header::RawEntryHeader};

pub(crate) struct DataFile<'a> {
    data: BlockSource<'a>,
}

impl<'a> DataFile<'a> {
    pub(crate) fn new(data: BlockSource<'a>) -> Self {
        DataFile { data }
    }

    pub(crate) async fn read_contents(
        &self,
        location: ResourceLocation,
    ) -> Result<Contents<'a>, Error> {
        if self.data.size() < u64::from(location.file_offset()) {
            return Err(Error::InvalidResourceLocation {
                location: location.file_offset() as usize,
                reason: "file offset is beyond end of file".into(),
            });
        }

        let (header, rest) = RawEntryHeader::from_block_source(
            &self.data.subblock(u64::from(location.file_offset())..),
        )
        .await?;

        let packed_size = u64::from(header.packed_size());

        if rest.size() < packed_size {
            return Err(Error::InvalidResourceLocation {
                location: location.file_offset() as usize,
                reason: format!("resource data ({packed_size} bytes) extends beyond end of file"),
            });
        }

        let resource_block = rest.subblock(..packed_size);
        Ok(Contents::from_parts(header, resource_block)?)
    }
}

#[cfg(test)]
mod tests {
    use crate::resources::{ResourceId, ResourceType};

    use super::*;
    use datalit::datalit;

    #[tokio::test]
    async fn test_read_data_file() {
        let data = datalit! {
            @endian = le,
            0x81,               // res_type
            100u16,             // res_number
            len('data): u16,    // packed_size
            len('data): u16,    // unpacked_size
            0u16,               // compression_type (none)
            'data: { 0xFADEDFAE },  // data
            // Should not include further bytes.
            0xDEADBEEF,
        };

        let id = ResourceId::new(ResourceType::Pic, 100);
        let location = ResourceLocation::new(id, 0);

        let data_file = DataFile::new(BlockSource::from_vec(data.to_vec()));
        let contents = data_file.read_contents(location).await.unwrap();
        assert_eq!(contents.id(), &id);
        let block = contents.data().open().await.unwrap();
        assert_eq!(block.as_ref(), &[0xFA, 0xDE, 0xDF, 0xAE]);
    }
}
