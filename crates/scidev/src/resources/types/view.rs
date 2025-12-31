use std::{
    collections::{BTreeMap, btree_map},
    io,
    sync::Arc,
};

use bytes::{Buf as _, BufMut as _};

use crate::{
    resources::types::palette::Palette,
    utils::{
        block::Block,
        mem_reader::{BufferMemReader, MemReader},
        range::BoundedRange,
    },
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
    pub fn read_from<M: MemReader>(reader: &mut M) -> io::Result<ViewHeader> {
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
            rest: header_data.read_remaining()?.into(),
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
    pub fn read_from<M: MemReader>(reader: &mut M) -> io::Result<LoopEntry> {
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
            rest: reader.read_remaining()?.into(),
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
    pub fn read_from<M: MemReader>(reader: &mut M) -> io::Result<CelEntry> {
        let width = reader.read_u16_le()?;
        let height = reader.read_u16_le()?;
        let displace_x = reader.read_i16_le()?;
        let displace_y = reader.read_i16_le()?;
        let clear_key = reader.read_u8()?;
        let mut reserved1 = [0u8; 15];
        reader.read_exact(&mut reserved1)?;
        let rle_offset = reader.read_u32_le()?;
        let literal_offset = reader.read_u32_le()?;
        let rest = reader.read_remaining()?;
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
    range_starts: BTreeMap<T, RangeId>,
}

impl<T> RangeComputer<T>
where
    T: Ord + Copy + num::PrimInt + num::Unsigned + std::fmt::Debug,
{
    fn new() -> Self {
        Self {
            next_range_id: 0,
            range_starts: BTreeMap::new(),
        }
    }

    fn add_range_start(&mut self, start: T) -> RangeId {
        match self.range_starts.entry(start) {
            btree_map::Entry::Vacant(vac) => {
                let id = RangeId {
                    id: self.next_range_id,
                };
                self.next_range_id += 1;
                vac.insert(id);
                id
            }
            btree_map::Entry::Occupied(occ) => *occ.get(),
        }
    }

    /// Assuming all blocks have been marked, defines a mapping from a range ID
    /// to ranges of the form [start, end)
    ///
    /// All ranges are assumed to be non-overlapping.
    fn get_ranges(&self, range_end: T) -> BTreeMap<RangeId, BoundedRange<T>> {
        let mut ranges = BTreeMap::new();
        let mut prev = None;
        for (&start, &id) in &self.range_starts {
            if let Some((prev_start, prev_id)) = prev {
                let range = BoundedRange::from_range(prev_start..start);
                ranges.insert(prev_id, range);
            }
            prev = Some((start, id));
        }
        if let Some((start, id)) = prev {
            ranges.insert(id, BoundedRange::from_range(start..range_end));
        }
        ranges
    }
}

/// Data that is shared between cel representations
#[derive(Debug, Clone)]
struct CelData {
    width: u16,
    height: u16,
    displace_x: i16,
    displace_y: i16,
    clear_key: u8,
}

#[derive(Debug, Clone)]
pub struct Cel {
    data: CelData,
    rle_block: Block,
    literal_block: Option<Block>,
}

impl Cel {
    #[must_use]
    pub fn height(&self) -> u16 {
        self.data.height
    }

    #[must_use]
    pub fn width(&self) -> u16 {
        self.data.width
    }

    #[must_use]
    pub fn displace_x(&self) -> i16 {
        self.data.displace_x
    }

    #[must_use]
    pub fn displace_y(&self) -> i16 {
        self.data.displace_y
    }

    #[must_use]
    pub fn clear_key(&self) -> u8 {
        self.data.clear_key
    }

    pub fn decode_pixels(&self) -> io::Result<Vec<u8>> {
        let num_pixels = usize::from(self.width()) * usize::from(self.height());
        let mut pixels = bytes::BytesMut::with_capacity(num_pixels).limit(num_pixels); // Initialize with transparent
        // SCI1.1 RLE decoder
        //
        // We potentially have to track two different pieces of data: The RLE stream and the literal stream.
        // If there is no literal stream, the literal data is encoded in the RLE stream.
        let rle_data = self.rle_block.open_mem(..).unwrap();
        let literal_data = self
            .literal_block
            .as_ref()
            .map(|block| block.open_mem(..).unwrap());

        // Given that we're only reading raw bytes, it's easier to use byte slices for parsing here.
        let mut rle_data = &rle_data[..];
        let mut literal_data = literal_data.as_ref().map(|d| &d[..]);

        while pixels.has_remaining_mut() {
            assert!(
                rle_data.has_remaining(),
                "RLE data has run out. Remaining pixels: {}. Cel: {self:#?}",
                pixels.remaining_mut()
            );
            let code = rle_data.get_u8();
            let has_high_bit = code & 0x80 != 0;
            if !has_high_bit {
                // Copy
                // The first 7 bits are the run length for copy operations. Since
                // the first bit is zero, we can just use the code as the run length.
                let run_length = usize::from(code);
                let mut src = if let Some(literal_data) = literal_data.as_mut() {
                    literal_data
                } else {
                    &mut rle_data
                };

                let copy_bytes = bytes::Buf::take(&mut src, run_length);
                pixels.put(copy_bytes);
                continue;
            }

            // This is some flavor of RLE.
            let action = code & 0x40;
            let run_length = usize::from(code & 0x3F);

            let color = if action == 0 {
                // Fill operation. Take fill color from available data.
                if let Some(literal_data) = literal_data.as_mut() {
                    literal_data.get_u8()
                } else {
                    rle_data.get_u8()
                }
            } else {
                // Skip (Transparent). Use the clear key.
                self.data.clear_key
            };
            pixels.put_bytes(color, run_length);
        }

        Ok(pixels.into_inner().into())
    }
}

#[derive(Debug, Clone)]
pub struct LoopData {
    cels: Vec<Cel>,
}

#[derive(Debug, Clone)]
pub struct Loop {
    #[expect(dead_code, reason = "Not used yet.")]
    mirrored: bool,
    loop_data: Arc<LoopData>,
}

impl Loop {
    #[must_use]
    pub fn cels(&self) -> &[Cel] {
        &self.loop_data.cels
    }
}

#[derive(Debug, Clone)]
pub struct View {
    #[expect(dead_code, reason = "Not used yet.")]
    flags: u8,
    palette: Option<Palette>,
    loops: Vec<Loop>,
}

#[derive(Debug)]
struct CelState {
    data: CelData,
    rle: RangeId,
    literal: Option<RangeId>,
}

/// Intermediate loop decoding state during parsing.
#[derive(Debug)]
struct LoopState {
    entry: LoopEntry,
    cels: Vec<CelState>,
}

impl LoopState {
    fn from_reader<M>(
        resource_block: &Block,
        header: &ViewHeader,
        loop_reader: &mut M,
        ranges: &mut RangeComputer<u32>,
    ) -> io::Result<Self>
    where
        M: MemReader,
    {
        let loop_entry = LoopEntry::read_from(loop_reader)?;

        // The cel data is indexed from the start of the loop data
        ranges.add_range_start(loop_entry.cel_offset);
        let cel_count = u64::from(loop_entry.cel_count);
        let cel_size = u64::from(header.cel_size);
        let cel_offset = u64::from(loop_entry.cel_offset);
        let cel_data = resource_block
            .subblock(cel_offset..)
            .subblock(..cel_count * cel_size);
        let mut cel_reader = BufferMemReader::new(cel_data.to_buffer().unwrap());

        let mut cels = Vec::with_capacity(usize::from(loop_entry.cel_count));
        for i in 0..cel_count {
            let entry = CelEntry::read_from(
                &mut cel_reader.read_to_subreader(format!("{i}"), usize::from(header.cel_size))?,
            )?;
            cels.push(CelState {
                data: CelData {
                    width: entry.width,
                    height: entry.height,
                    displace_x: entry.displace_x,
                    displace_y: entry.displace_y,
                    clear_key: entry.clear_key,
                },
                rle: ranges.add_range_start(entry.rle_offset),
                literal: if entry.literal_offset != 0 {
                    Some(ranges.add_range_start(entry.literal_offset))
                } else {
                    None
                },
            });
        }
        Ok(LoopState {
            entry: loop_entry,
            cels,
        })
    }
}

impl View {
    #[must_use]
    pub fn palette(&self) -> Option<&Palette> {
        self.palette.as_ref()
    }

    #[must_use]
    pub fn loops(&self) -> &[Loop] {
        &self.loops
    }

    pub fn from_resource(resource_data: &Block) -> io::Result<View> {
        // Keep track of ranges of data in the view
        let mut ranges = RangeComputer::<u32>::new();
        ranges.add_range_start(0);
        let buffer = resource_data.to_buffer().unwrap();
        let mut reader = BufferMemReader::new(buffer.clone());
        let header = ViewHeader::read_from(&mut reader)?;
        let loop_count = usize::from(header.loop_count);
        let loop_size = usize::from(header.loop_size);
        let mut loop_reader = reader.read_to_subreader("loop_data", loop_count * loop_size)?;
        let palette_range = if header.pal_offset != 0 {
            Some(ranges.add_range_start(header.pal_offset))
        } else {
            None
        };

        let mut loop_states = Vec::with_capacity(loop_count);
        for i in 0..loop_count {
            let loop_state = LoopState::from_reader(
                resource_data,
                &header,
                &mut loop_reader.read_to_subreader(format!("{i}"), loop_size)?,
                &mut ranges,
            )?;
            loop_states.push(loop_state);
        }

        let ranges = ranges.get_ranges(resource_data.len().try_into().unwrap());

        let palette = palette_range
            .map(|range| {
                Palette::from_data(
                    resource_data
                        .subblock(ranges.get(&range).unwrap().cast_to::<u64>())
                        .open_mem(..)
                        .unwrap(),
                )
            })
            .transpose()
            .unwrap();

        let loop_data = loop_states
            .iter()
            .map(|loop_state| {
                let cels = loop_state
                    .cels
                    .iter()
                    .map(|cel_state| {
                        let rle_block = resource_data
                            .subblock(ranges.get(&cel_state.rle).unwrap().cast_to::<u64>());
                        let literal_block = cel_state.literal.map(|range| {
                            resource_data.subblock(ranges.get(&range).unwrap().cast_to::<u64>())
                        });
                        Cel {
                            data: cel_state.data.clone(),
                            rle_block,
                            literal_block,
                        }
                    })
                    .collect::<Vec<_>>();
                Arc::new(LoopData { cels })
            })
            .collect::<Vec<_>>();

        let loops = loop_states
            .into_iter()
            .enumerate()
            .map(|(i, loop_state)| {
                let (mirrored, loop_data) = if loop_state.entry.seek_entry == 255 {
                    (false, loop_data.get(i).unwrap().clone())
                } else {
                    (
                        true,
                        loop_data
                            .get(usize::from(loop_state.entry.seek_entry))
                            .unwrap()
                            .clone(),
                    )
                };
                Loop {
                    mirrored,
                    loop_data,
                }
            })
            .collect::<Vec<_>>();

        Ok(Self {
            flags: header.flags,
            palette,
            loops,
        })
    }
}
