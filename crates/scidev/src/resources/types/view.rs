#![expect(dead_code, reason = "Views will be used in a near-future version.")]

use crate::utils::mem_reader::{self, MemReader};

#[derive(Debug)]
pub(crate) struct ViewHeader {
    pub loop_count: u8,
    pub flags: u8,
    pub reserved: [u8; 4],
    pub pal_offset: u32,
    pub loop_size: u8,
    pub cel_size: u8,
    pub rest: Vec<u8>,
}

impl ViewHeader {
    pub(crate) fn read_from<M: MemReader>(reader: &mut M) -> mem_reader::Result<ViewHeader> {
        let header_size = reader.read_u16_le()?;
        let mut header_data = reader.read_to_subreader("view_header", header_size.into())?;
        let loop_count = header_data.read_u8()?;
        let flags = header_data.read_u8()?;
        let mut reserved = [0u8; 4];
        header_data.read_exact(&mut reserved)?;
        let pal_offset = header_data.read_u32_le()?;
        let loop_size = header_data.read_u8()?;
        let cel_size = header_data.read_u8()?;

        Ok(ViewHeader {
            loop_count,
            flags,
            reserved,
            pal_offset,
            loop_size,
            cel_size,
            rest: header_data
                .read_remaining()
                .map_err(mem_reader::MemReaderError::Read)?,
        })
    }
}

#[derive(Debug)]
pub(crate) struct LoopEntry {
    pub seek_entry: u8,
    pub reserved1: u8,
    pub cel_count: u8,
    pub reserved2: [u8; 9],
    pub cel_offset: u32,
    pub rest: Vec<u8>,
}

impl LoopEntry {
    pub(crate) fn read_from<M: MemReader>(reader: &mut M) -> mem_reader::Result<LoopEntry> {
        let seek_entry = reader.read_u8()?;
        let reserved1 = reader.read_u8()?;
        let cel_count = reader.read_u8()?;
        let mut reserved2 = [0u8; 9];
        reader.read_exact(&mut reserved2)?;
        let cel_offset = reader.read_u32_le()?;
        Ok(LoopEntry {
            seek_entry,
            reserved1,
            cel_count,
            reserved2,
            cel_offset,
            rest: reader
                .read_remaining()
                .map_err(mem_reader::MemReaderError::Read)?,
        })
    }
}

#[derive(Debug)]
pub struct CelEntry {
    pub width: u16,
    pub height: u16,
    pub displace_x: i16,
    pub displace_y: i16,
    pub clear_key: u8,
    pub reserved1: [u8; 15],
    pub rle_offset: u32,
    pub literal_offset: u32,
    pub rest: Vec<u8>,
}

impl CelEntry {
    pub fn read_from<M: MemReader>(reader: &mut M) -> mem_reader::Result<CelEntry> {
        let width = reader.read_u16_le()?;
        let height = reader.read_u16_le()?;
        let displace_x = reader.read_i16_le()?;
        let displace_y = reader.read_i16_le()?;
        let clear_key = reader.read_u8()?;
        let mut reserved1 = [0u8; 15];
        reader.read_exact(&mut reserved1)?;
        let rle_offset = reader.read_u32_le()?;
        let literal_offset = reader.read_u32_le()?;
        let rest = reader
            .read_remaining()
            .map_err(mem_reader::MemReaderError::Read)?;
        Ok(CelEntry {
            width,
            height,
            displace_x,
            displace_y,
            clear_key,
            reserved1,
            rle_offset,
            literal_offset,
            rest,
        })
    }
}
