use std::{
    collections::BTreeMap,
    io::{self, Cursor},
};

use crate::utils::{
    block::{BlockSource, LazyBlock, MemBlock, OutputBlock},
    data_writer::{DataWriter, IoDataWriter},
    errors::{OtherError, ensure_other, prelude::*},
    mem_reader::{self, MemReader},
};
use bytes::BufMut;

use crate::resources::{ResourceId, ResourceType, file::Resource};

use super::msg::MessageId;

/// A map entry for the audio36 map file.
///
/// This is based on the early SCI1.1 audio36 map file format.
struct RawMapEntry {
    id: MessageId,
    offset: u32,
    sync_size: u16,
}

impl RawMapEntry {
    pub(crate) fn read_from<M: MemReader>(
        reader: &mut M,
    ) -> mem_reader::Result<RawMapEntry, M::Error> {
        let noun = reader.read_u8()?;
        let verb = reader.read_u8()?;
        let cond = reader.read_u8()?;
        let seq = reader.read_u8()?;
        let offset = reader.read_u32_le()?;
        let sync_size = reader.read_u16_le()?;
        Ok(RawMapEntry {
            id: MessageId::new(noun, verb, cond, seq),
            offset,
            sync_size,
        })
    }

    pub(crate) fn write_to<W: DataWriter>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_u8(self.id.noun())?;
        writer.write_u8(self.id.verb())?;
        writer.write_u8(self.id.condition())?;
        writer.write_u8(self.id.sequence())?;
        writer.write_u32_le(self.offset)?;
        writer.write_u16_le(self.sync_size)?;
        Ok(())
    }
}

struct RawMapResource {
    entries: Vec<RawMapEntry>,
}

impl RawMapResource {
    pub(crate) fn new() -> Self {
        RawMapResource {
            entries: Vec::new(),
        }
    }

    pub(crate) fn add_entry(&mut self, id: MessageId, offset: u32) {
        // We don't currently support the sync size, so we just set it to 0.
        let sync_size = 0;
        self.entries.push(RawMapEntry {
            id,
            offset,
            sync_size,
        });
    }

    #[expect(dead_code)]
    pub(crate) fn read_from<M: MemReader>(
        reader: &mut M,
    ) -> mem_reader::Result<RawMapResource, M::Error> {
        let mut entries = Vec::new();
        loop {
            let entry = RawMapEntry::read_from(reader)?;
            if entry.id.noun() == 0xFF {
                break;
            }
            entries.push(entry);
        }
        Ok(RawMapResource { entries })
    }

    pub(crate) fn write_to<W: DataWriter>(&self, writer: &mut W) -> io::Result<()> {
        const TERM_BYTES: &[u8] = &[0xFF; 10];

        for entry in &self.entries {
            entry.write_to(writer)?;
        }
        // Write the terminator entry, consisting of an entry of all 0xFFs.
        writer.write_slice(TERM_BYTES)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AudioFormat {
    Mp3,
    Flac,
    Ogg,
    Wav,
}

pub struct VoiceSample {
    format: AudioFormat,
    data: BlockSource,
}

impl VoiceSample {
    #[must_use]
    pub fn new(format: AudioFormat, data: BlockSource) -> Self {
        VoiceSample { format, data }
    }

    #[must_use]
    pub fn format(&self) -> AudioFormat {
        self.format
    }

    #[must_use]
    pub fn data(&self) -> &BlockSource {
        &self.data
    }
}

struct AudioVolumeEntry {
    logical_offset: u32,
    data: BlockSource,
}

struct AudioVolumeBuilder {
    format: Option<AudioFormat>,
    entries: Vec<AudioVolumeEntry>,
    curr_offset: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum AudioVolumeBuilderError {
    #[doc(hidden)]
    #[error(transparent)]
    Other(#[from] OtherError),
}

impl AudioVolumeBuilder {
    pub(crate) fn new() -> Self {
        AudioVolumeBuilder {
            format: None,
            entries: Vec::new(),
            curr_offset: 0,
        }
    }

    pub(crate) fn add_entry(
        &mut self,
        sample: &VoiceSample,
    ) -> Result<u32, AudioVolumeBuilderError> {
        // Check if the entry is vaild. Variable is copied in case we need to use it to
        // calculate the new file offset.
        let _format = if let Some(format) = self.format {
            ensure_other!(
                format == sample.format,
                "Audio format mismatch: expected {:?}, got {:?}",
                format,
                sample.format
            );
            format
        } else {
            let format = sample.format;
            self.format = Some(sample.format);
            format
        };
        let logical_offset = self.curr_offset;
        let data = sample.data.clone();

        // The offset of the next entry depends on the audio format, aas well as the size of
        // the current entry.
        //
        // WAV files are simply written directly to the file, as it contains all information about
        // the length of the file.
        //
        // For other compressed formats, we initial compressed map table will provide the lengths,
        // but also provides the mapping from the logical offset to the actual offset in the
        // compressed file, so we can choose any value as long as it mapped in the table.
        //
        // To keep things simple, assume we are using one of these two options, which means if we
        // can just use the size of the current entry as the size of this entry.
        #[expect(clippy::cast_possible_truncation)]
        {
            self.curr_offset += data.size() as u32;
        }
        self.entries.push(AudioVolumeEntry {
            logical_offset,
            data,
        });
        Ok(logical_offset)
    }

    #[expect(clippy::cast_possible_truncation)]
    fn header_size(&self) -> u32 {
        4 + // The size of the 4CC header (e.g. "MP3 " or "FLAC")
        4 + // The number of entries in the compressed volume table
        (8 * self.entries.len() as u32) // The size of all the entries
    }

    fn to_raw_of_compressed_format(&self, archive_type: [u8; 4]) -> OutputBlock {
        let mut header_bytes = bytes::BytesMut::new();
        header_bytes.extend_from_slice(&archive_type);
        #[expect(clippy::cast_possible_truncation)]
        header_bytes.put_u32_le(self.entries.len() as u32);
        let mut curr_data_offset = self.header_size();
        for entry in &self.entries {
            header_bytes.put_u32_le(entry.logical_offset);
            header_bytes.put_u32_le(curr_data_offset);
            curr_data_offset += u32::try_from(entry.data.size()).unwrap();
        }
        #[expect(clippy::cast_possible_truncation)]
        {
            assert_eq!(self.header_size(), header_bytes.len() as u32);
        }
        let header: OutputBlock = header_bytes.freeze().into();
        let mut volume_blocks = Vec::new();
        volume_blocks.push(header);
        for entry in &self.entries {
            volume_blocks.push(OutputBlock::from_block_source(entry.data.clone()));
        }
        volume_blocks.into_iter().collect()
    }

    pub(crate) fn to_raw(&self) -> OutputBlock {
        match self.format {
            Some(AudioFormat::Mp3) => self.to_raw_of_compressed_format(*b"MP3 "),
            Some(AudioFormat::Flac) => self.to_raw_of_compressed_format(*b"FLAC"),
            Some(AudioFormat::Ogg) => self.to_raw_of_compressed_format(*b"OGG "),
            Some(AudioFormat::Wav) => {
                // WAV files are not treated as compressed, so we can just
                // concatenate the entries together.
                let mut volume_blocks = Vec::new();
                for entry in &self.entries {
                    volume_blocks.push(OutputBlock::from_block_source(entry.data.clone()));
                }
                volume_blocks.into_iter().collect()
            }
            None => OutputBlock::from_buffer(MemBlock::from_vec(vec![])),
        }
    }
}

impl Default for AudioVolumeBuilder {
    fn default() -> Self {
        AudioVolumeBuilder::new()
    }
}

pub struct Audio36ResourceBuilder {
    maps: BTreeMap<u16, RawMapResource>,
    volume: AudioVolumeBuilder,
}

impl Audio36ResourceBuilder {
    #[must_use]
    pub fn new() -> Self {
        Audio36ResourceBuilder {
            maps: BTreeMap::new(),
            volume: AudioVolumeBuilder::new(),
        }
    }

    pub fn add_entry(
        &mut self,
        room: u16,
        entry: MessageId,
        sample: &VoiceSample,
    ) -> Result<(), AudioVolumeBuilderError> {
        let offset: u32 = self.volume.add_entry(sample)?;

        let resource_map = self.maps.entry(room).or_insert_with(RawMapResource::new);

        resource_map.add_entry(entry, offset);

        Ok(())
    }

    pub fn build(self) -> Result<VoiceSampleResources, AudioVolumeBuilderError> {
        let mut map_resources = Vec::new();
        for (room, map) in self.maps {
            let mut map_data = Vec::new();
            map.write_to(&mut IoDataWriter::new(&mut Cursor::new(&mut map_data)))
                .with_other_err()?;
            map_resources.push(Resource::new(
                ResourceId::new(ResourceType::Map, room),
                LazyBlock::from_mem_block(MemBlock::from_vec(map_data)),
            ));
        }

        let audio_volume = self.volume.to_raw();

        Ok(VoiceSampleResources {
            map_resources,
            audio_volume,
        })
    }
}

impl Default for Audio36ResourceBuilder {
    fn default() -> Self {
        Audio36ResourceBuilder::new()
    }
}

#[derive(Clone)]
pub struct VoiceSampleResources {
    map_resources: Vec<Resource>,
    audio_volume: OutputBlock,
}

impl VoiceSampleResources {
    #[must_use]
    pub fn map_resources(&self) -> &[Resource] {
        &self.map_resources
    }

    #[must_use]
    pub fn audio_volume(&self) -> &OutputBlock {
        &self.audio_volume
    }
}
