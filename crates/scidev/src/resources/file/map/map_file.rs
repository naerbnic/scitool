use std::{io, sync::Arc};

use crate::{
    resources::file::map::{ResourceLocation, ResourceLocationSet},
    utils::{
        buffer::ReaderBuffer,
        mem_reader::{self, BufferMemReader, Parse as _},
    },
};

pub(crate) struct MapFile {
    locations: ResourceLocationSet,
}

impl MapFile {
    pub(crate) fn from_read_seek<R>(reader: R) -> std::io::Result<Self>
    where
        R: std::io::Read + std::io::Seek,
    {
        let buffer = Arc::new(ReaderBuffer::new(reader)?);
        let locations = ResourceLocationSet::parse(&mut BufferMemReader::new(buffer.clone()))
            .map_err(|e| match e {
                mem_reader::MemReaderError::Read(e) => e,
                mem_reader::MemReaderError::InvalidData(err) => io::Error::other(err),
            })?;
        Ok(MapFile { locations })
    }

    pub(crate) fn locations(&self) -> impl Iterator<Item = ResourceLocation> {
        self.locations.locations()
    }
}
