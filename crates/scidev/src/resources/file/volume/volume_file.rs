use crate::{
    resources::{
        ResourceId,
        file::{map::ResourceLocation, volume::raw_contents::RawContents},
    },
    utils::{
        block::{Block, FromBlock as _},
        errors::{OtherError, OtherResultExt},
    },
};

#[derive(Debug, thiserror::Error)]
#[error("Resource ID mismatch: expected {expected:?}, got {got:?}")]
pub(super) struct ResourceIdMismatch {
    pub expected: ResourceId,
    pub got: ResourceId,
}

#[derive(Debug, thiserror::Error)]
#[error("Invalid resource location {location:x}: {reason}")]
pub(super) struct InvalidResourceLocation {
    pub location: usize,
    pub reason: String,
}

use super::{contents::Contents, raw_header::RawEntryHeader};

pub(crate) struct VolumeFile {
    data: Block,
}

impl VolumeFile {
    pub(crate) fn new(data: Block) -> Self {
        VolumeFile { data }
    }

    pub(crate) fn read_raw_contents(&self, offset: u32) -> Result<RawContents, OtherError> {
        if self.data.len() < u64::from(offset) {
            return Err(OtherError::new(InvalidResourceLocation {
                location: offset as usize,
                reason: "file offset is beyond end of file".into(),
            }));
        }

        let (header, rest) =
            RawEntryHeader::from_block_source(&self.data.subblock(u64::from(offset)..))?;

        let packed_size = u64::from(header.packed_size());

        if rest.len() < packed_size {
            return Err(OtherError::new(InvalidResourceLocation {
                location: offset as usize,
                reason: format!("resource data ({packed_size} bytes) extends beyond end of file"),
            }));
        }

        let resource_block = rest.subblock(..packed_size);
        Ok(RawContents::new(offset, header, resource_block))
    }

    pub(crate) fn read_contents(&self, location: ResourceLocation) -> Result<Contents, OtherError> {
        let raw_contents = &self.read_raw_contents(location.file_offset())?;
        if raw_contents.id() != location.id() {
            return Err(OtherError::new(ResourceIdMismatch {
                expected: location.id(),
                got: raw_contents.id(),
            }));
        }
        Contents::from_raw(raw_contents).with_other_err()
    }
}

#[cfg(test)]
mod tests {
    use crate::resources::{ResourceType};

    use super::*;
    use datalit::datalit;

    #[test]
    fn test_read_data_file() {
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

        let data_file = VolumeFile::new(Block::from_vec(data.to_vec()));
        let contents = data_file.read_contents(location).unwrap();
        let block = contents.data().open_mem(..).unwrap();
        assert_eq!(block.as_ref(), &[0xFA, 0xDE, 0xDF, 0xAE]);
    }
}
