use std::io;

use crate::{
    res::{ResourceId, ResourceType},
    util::{
        block::{BlockSource, LazyBlock},
        compression::dcl::decompress_dcl,
        data_reader::{DataReader, FromBlockSource},
    },
};

use super::map::ResourceLocation;

/// A resource entry header in a data file.
///
/// This is based on the SCI1.1 data file format.
#[derive(Debug)]
pub struct RawEntryHeader {
    res_type: u8,
    res_number: u16,
    packed_size: u16,
    unpacked_size: u16,
    compression_type: u16,
}

impl FromBlockSource for RawEntryHeader {
    fn read_size() -> usize {
        9
    }

    fn parse<R>(mut reader: R) -> io::Result<Self>
    where
        R: DataReader,
    {
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

pub struct RawContents {
    res_type: u8,
    res_number: u16,
    unpacked_size: u16,
    compression_type: u16,
    data: BlockSource,
}

impl std::fmt::Debug for RawContents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RawContents")
            .field("res_type", &self.res_type)
            .field("res_number", &self.res_number)
            .field("unpacked_size", &self.unpacked_size)
            .field("compression_type", &self.compression_type)
            .field("data", &self.data.size())
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct Contents {
    id: ResourceId,
    data: LazyBlock,
}

impl Contents {
    pub fn id(&self) -> &ResourceId {
        &self.id
    }
    pub fn data(&self) -> &LazyBlock {
        &self.data
    }
}

impl TryFrom<RawContents> for Contents {
    type Error = io::Error;

    fn try_from(raw_contents: RawContents) -> Result<Self, Self::Error> {
        let decompressed_data = match raw_contents.compression_type {
            0 => raw_contents.data.to_lazy_block(),
            18 => raw_contents
                .data
                .to_lazy_block()
                .map(move |block| Ok(decompress_dcl(&block, raw_contents.unpacked_size as usize)?)),
            _ => {
                // Let's be lazy here.
                LazyBlock::from_factory(move || {
                    Err(io::Error::other(format!(
                        "Unsupported compression type: {}",
                        raw_contents.compression_type
                    ))
                    .into())
                })
            }
        };
        let decompressed_data = decompressed_data.with_check(move |block| {
            if block.size() != raw_contents.unpacked_size as u64 {
                return Err(io::Error::other("Decompressed data size mismatch").into());
            }
            Ok(())
        });

        Ok(Contents {
            id: ResourceId::new(
                ResourceType::try_from(raw_contents.res_type).map_err(io::Error::other)?,
                raw_contents.res_number,
            ),
            data: decompressed_data,
        })
    }
}

pub struct DataFile {
    data: BlockSource,
}

impl DataFile {
    pub fn new(data: BlockSource) -> DataFile {
        DataFile { data }
    }

    pub fn read_raw_contents(&self, location: &ResourceLocation) -> io::Result<RawContents> {
        let (header, rest) =
            RawEntryHeader::from_block_source(&self.data.subblock(location.file_offset as u64..))?;
        let resource_block = rest.subblock(..header.packed_size as u64);
        assert_eq!(resource_block.size(), header.packed_size as u64);
        Ok(RawContents {
            res_type: header.res_type,
            res_number: header.res_number,
            unpacked_size: header.unpacked_size,
            compression_type: header.compression_type,
            data: resource_block,
        })
    }

    pub fn read_contents(&self, location: &ResourceLocation) -> io::Result<Contents> {
        let raw_contents = self.read_raw_contents(location)?;
        raw_contents.try_into()
    }
}
