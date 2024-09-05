use std::io;

use crate::util::{data_reader::DataReader, data_writer::DataWriter};

/// A map entry for the audio36 map file.
///
/// This is based on the early SCI1.1 audio36 map file format.
pub struct RawMapEntry {
    pub noun: u8,
    pub verb: u8,
    pub cond: u8,
    pub seq: u8,
    pub offset: u32,
    pub sync_size: u16,
}

impl RawMapEntry {
    pub fn new_terminator_entry() -> RawMapEntry {
        RawMapEntry {
            noun: 0xFF,
            verb: 0xFF,
            cond: 0xFF,
            seq: 0xFF,
            offset: 0xFFFF_FFFF,
            sync_size: 0xFFFF,
        }
    }
    pub fn read_from<R: DataReader>(reader: &mut R) -> io::Result<RawMapEntry> {
        let noun = reader.read_u8()?;
        let verb = reader.read_u8()?;
        let cond = reader.read_u8()?;
        let seq = reader.read_u8()?;
        let offset = reader.read_u32_le()?;
        let sync_size = reader.read_u16_le()?;
        Ok(RawMapEntry {
            noun,
            verb,
            cond,
            seq,
            offset,
            sync_size,
        })
    }

    pub fn write_to<W: DataWriter>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_u8(self.noun)?;
        writer.write_u8(self.verb)?;
        writer.write_u8(self.cond)?;
        writer.write_u8(self.seq)?;
        writer.write_u32_le(self.offset)?;
        writer.write_u16_le(self.sync_size)?;
        Ok(())
    }
}

pub struct RawMapResource {
    pub entries: Vec<RawMapEntry>,
}

impl RawMapResource {
    pub fn read_from<R: DataReader>(reader: &mut R) -> io::Result<RawMapResource> {
        let mut entries = Vec::new();
        loop {
            let entry = RawMapEntry::read_from(reader)?;
            if entry.noun == 0xFF {
                break;
            }
            entries.push(entry);
        }
        Ok(RawMapResource { entries })
    }

    pub fn write_to<W: DataWriter>(&self, writer: &mut W) -> io::Result<()> {
        for entry in &self.entries {
            entry.write_to(writer)?;
        }
        RawMapEntry::new_terminator_entry().write_to(writer)?;
        Ok(())
    }
}

pub enum AudioDataEntry {
    /// A raw WAV file. Written directly to the output.
    WaveFile(Vec<u8>),
    // Other variants are not yet supported, and likely not necessary for our use here.
}

pub struct Audio36Data {
    pub map: RawMapResource,
    pub audio_data: AudioDataEntry,
}
