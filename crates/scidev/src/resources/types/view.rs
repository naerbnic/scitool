use std::{collections::BTreeMap, sync::Arc};

use crate::utils::{
    block::Block,
    buffer::{Buffer, ReaderBuffer},
    mem_reader::{self, BufferMemReader, MemReader},
};

fn encode_ascii_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .flat_map(|&b| std::ascii::escape_default(b).map(|c| c as char))
        .collect::<String>()
}

/// Helper data structure to make easy-to-print data blocks for fixed sizes.
pub struct RawSizedData<const N: usize>([u8; N]);

impl<const N: usize> std::fmt::Debug for RawSizedData<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("RawSizedData")
            .field(&format_args!("b\"{}\"", encode_ascii_bytes(&self.0)))
            .finish()
    }
}

impl<const N: usize> From<[u8; N]> for RawSizedData<N> {
    fn from(data: [u8; N]) -> Self {
        Self(data)
    }
}

/// Helper data structure to make easy-to-print data blocks for dynamic sizes.
pub struct RawData(Vec<u8>);

impl std::fmt::Debug for RawData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("RawData")
            .field(&format_args!("b\"{}\"", encode_ascii_bytes(&self.0)))
            .finish()
    }
}

impl From<Vec<u8>> for RawData {
    fn from(data: Vec<u8>) -> Self {
        Self(data)
    }
}

#[derive(Debug)]
pub struct ViewHeader {
    pub loop_count: u8,
    pub flags: u8,
    pub reserved: RawSizedData<4>,
    pub pal_offset: u32,
    pub loop_size: u8,
    pub cel_size: u8,
    pub rest: RawData,
}

impl ViewHeader {
    pub fn read_from<M: MemReader>(reader: &mut M) -> mem_reader::Result<ViewHeader> {
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
            reserved: reserved.into(),
            pal_offset,
            loop_size,
            cel_size,
            rest: header_data
                .read_remaining()
                .map_err(mem_reader::MemReaderError::Read)?
                .into(),
        })
    }
}

#[derive(Debug)]
pub struct LoopEntry {
    pub seek_entry: u8,
    pub reserved1: u8,
    pub cel_count: u8,
    pub reserved2: RawSizedData<9>,
    pub cel_offset: u32,
    pub rest: RawData,
}

impl LoopEntry {
    pub fn read_from<M: MemReader>(reader: &mut M) -> mem_reader::Result<LoopEntry> {
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
            reserved2: reserved2.into(),
            cel_offset,
            rest: reader
                .read_remaining()
                .map_err(mem_reader::MemReaderError::Read)?
                .into(),
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
    pub reserved1: RawSizedData<15>,
    pub rle_offset: u32,
    pub literal_offset: u32,
    pub rest: RawData,
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
            reserved1: reserved1.into(),
            rle_offset,
            literal_offset,
            rest: rest.into(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct RangeId {
    id: u32,
}

struct RangeComputer<T> {
    next_range_id: u32,
    block_size: T,
    range_starts: BTreeMap<T, RangeId>,
}

impl<T> RangeComputer<T>
where
    T: Ord + Copy,
{
    pub fn with_block_size(block_size: T) -> Self {
        Self {
            next_range_id: 0,
            block_size,
            range_starts: BTreeMap::new(),
        }
    }

    pub fn add_range_start(&mut self, start: T) -> RangeId {
        let id = RangeId {
            id: self.next_range_id,
        };
        self.next_range_id += 1;
        self.range_starts.insert(start, id);
        id
    }

    /// Assuming all blocks have been marked, defines a mapping from a range ID
    /// to ranges of the form [start, end)
    ///
    /// All ranges are assumed to be non-overlapping.
    pub fn get_ranges(&self) -> BTreeMap<RangeId, (T, T)> {
        let mut ranges = BTreeMap::new();
        let mut prev_start = None;
        let mut prev_id = None;
        for (&start, &id) in &self.range_starts {
            if let Some(prev_start) = prev_start {
                ranges.insert(prev_id.unwrap(), (prev_start, start));
            }
            prev_start = Some(start);
            prev_id = Some(id);
        }
        ranges
    }
}

#[derive(Debug, Clone)]
pub struct Cel {
    width: u16,
    height: u16,
    displace_x: i16,
    displace_y: i16,
    clear_key: u8,
    rle_block: Block,
    literal_block: Option<Block>,
}

#[derive(Debug, Clone)]
pub struct LoopData {
    cels: Vec<Cel>,
}

#[derive(Debug, Clone)]
pub struct Loop {
    mirrored: bool,
    loop_data: Arc<LoopData>,
}

pub struct View {
    flags: u8,
    palette_block: Block,
    loops: Vec<Loop>,
}

impl View {
    pub fn from_resource(resource_data: Block) -> mem_reader::Result<View> {
        todo!()
    }
}
