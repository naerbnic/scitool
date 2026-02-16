use std::sync::Arc;

use scidev_errors::{AnyDiag, prelude::*};

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
    pub(crate) fn from_read_seek<R>(reader: R) -> Result<Self, AnyDiag>
    where
        R: std::io::Read + std::io::Seek,
    {
        let buffer = Arc::new(
            ReaderBuffer::new(reader)
                .raise()
                .msg("IO error when creating map file")?,
        );
        let locations = ResourceLocationSet::parse(&mut BufferMemReader::new(buffer.clone()))
            .raise()
            .msg("Error while parsing map file")?;
        Ok(MapFile { locations })
    }

    pub(crate) fn locations(&self) -> impl Iterator<Item = ResourceLocation> {
        self.locations.locations()
    }
}
