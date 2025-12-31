use std::{io, sync::Arc};

use crate::{
    resources::file::map::{ResourceLocation, ResourceLocationSet},
    utils::{
        buffer::ReaderBuffer,
        mem_reader::{BufferMemReader, Parse as _},
    },
};

pub(crate) struct MapFile {
    locations: ResourceLocationSet,
}

impl MapFile {
    pub(crate) fn from_read_seek<R>(reader: R) -> io::Result<Self>
    where
        R: std::io::Read + std::io::Seek,
    {
        let buffer = Arc::new(ReaderBuffer::new(reader)?);
        let locations = ResourceLocationSet::parse(&mut BufferMemReader::new(buffer.clone()))?;
        Ok(MapFile { locations })
    }

    pub(crate) fn locations(&self) -> impl Iterator<Item = ResourceLocation> {
        self.locations.locations()
    }
}
