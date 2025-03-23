use std::io;

use sci_utils::{data_reader::DataReader, data_writer::DataWriter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntryId {
    noun: u8,
    verb: u8,
    cond: u8,
    seq: u8,
}

impl EntryId {
    pub fn new(noun: u8, verb: u8, cond: u8, seq: u8) -> EntryId {
        EntryId {
            noun,
            verb,
            cond,
            seq,
        }
    }

    pub fn noun(&self) -> u8 {
        self.noun
    }

    pub fn verb(&self) -> u8 {
        self.verb
    }

    pub fn cond(&self) -> u8 {
        self.cond
    }

    pub fn seq(&self) -> u8 {
        self.seq
    }
}

/// A map entry for the audio36 map file.
///
/// This is based on the early SCI1.1 audio36 map file format.
struct RawMapEntry {
    id: EntryId,
    pub offset: u32,
    pub sync_size: u16,
}

impl RawMapEntry {
    pub fn read_from<R: DataReader>(reader: &mut R) -> io::Result<RawMapEntry> {
        let noun = reader.read_u8()?;
        let verb = reader.read_u8()?;
        let cond = reader.read_u8()?;
        let seq = reader.read_u8()?;
        let offset = reader.read_u32_le()?;
        let sync_size = reader.read_u16_le()?;
        Ok(RawMapEntry {
            id: EntryId::new(noun, verb, cond, seq),
            offset,
            sync_size,
        })
    }

    pub fn write_to<W: DataWriter>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_u8(self.id.noun())?;
        writer.write_u8(self.id.verb())?;
        writer.write_u8(self.id.cond())?;
        writer.write_u8(self.id.seq())?;
        writer.write_u32_le(self.offset)?;
        writer.write_u16_le(self.sync_size)?;
        Ok(())
    }
}

pub struct RawMapResource {
    entries: Vec<RawMapEntry>,
}

impl RawMapResource {
    pub fn read_from<R: DataReader>(reader: &mut R) -> io::Result<RawMapResource> {
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

    pub fn write_to<W: DataWriter>(&self, writer: &mut W) -> io::Result<()> {
        for entry in &self.entries {
            entry.write_to(writer)?;
        }
        // Write the terminator entry, consisting of an entry of all 0xFFs.
        const TERM_BYTES: &[u8] = &[0xFF, 10];
        writer.write_slice(TERM_BYTES)?;
        Ok(())
    }
}


