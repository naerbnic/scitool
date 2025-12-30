//! Parsers and builders for Aseprite files.
//!
//! Using the spec at: <https://github.com/aseprite/aseprite/blob/main/docs/ase-file-specs.md>

#![expect(dead_code)]

use std::{
    collections::{BTreeMap, BTreeSet, btree_map},
    io,
    num::NonZeroU32,
    sync::Arc,
};

use bitflags::bitflags;
use scidev::utils::{
    block::{Block, BlockBuilder, BlockBuilderFactory},
    mem_reader::{self, MemReader, Result as MemResult},
};

use crate::formats::aseprite::{
    ColorDepth, FixedPoint, Point, Rect, Size, backing::SpriteContents,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum UserDataPropsKey {
    General,
    ForExtension(NonZeroU32),
}

fn read_string_type<M>(reader: &mut M) -> mem_reader::Result<String>
where
    M: MemReader,
{
    let byte_count = reader.read_u16_le()?;
    let mut bytes = vec![0u8; byte_count as usize];
    reader.read_exact(&mut bytes)?;
    let string = String::from_utf8(bytes)
        .map_err(|_| reader.create_invalid_data_error_msg("Invalid UTF-8"))?;
    Ok(string)
}

fn write_string_to(string: &str, builder: &mut BlockBuilder) -> io::Result<()> {
    let byte_count = u16::try_from(string.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "String too long"))?;
    builder.write_u16_le(byte_count)?;
    builder.write_bytes(string.as_bytes())?;
    Ok(())
}

/// An identifier for an extension.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) struct ExtensionId(u32);

impl ExtensionId {
    pub(super) fn to_u32(self) -> u32 {
        self.0
    }
}

/// Context for mapping extension names to IDs during serialization.
#[derive(Debug)]
pub(super) struct ExtensionContext {
    name_mapping: BTreeMap<String, ExtensionId>,
}

impl ExtensionContext {
    pub(super) fn from_sprite(sprite: &SpriteContents) -> Self {
        // Visit all UserData values in sprite and its children to collect
        // extension uses.
        let mut extension_set = BTreeSet::new();
        sprite.visit_user_data(|ud| {
            extension_set.extend(ud.extension_names());
        });

        let mut curr_id = 1;
        let mut name_mapping = BTreeMap::new();

        for name in extension_set {
            if let btree_map::Entry::Vacant(v) = name_mapping.entry(name.to_string()) {
                let entry_id = curr_id;
                curr_id += 1;
                v.insert(ExtensionId(entry_id));
            }
        }

        ExtensionContext { name_mapping }
    }

    pub(super) fn extension_mappings(&self) -> impl Iterator<Item = (&String, &ExtensionId)> {
        self.name_mapping.iter()
    }
}

bitflags! {
    /// Flags for Aseprite frames.
    pub struct HeaderFlags: u32 {
        /// Layer opacity is valid.
        const HAS_LAYER_OPACITY = 0x0001;
        /// Layer group blend mode is valid.
        const HAS_LAYER_GROUP_BLEND = 0x0002;
        /// Layers have UUIDs.
        const HAS_LAYER_UUIDS = 0x0004;
    }
}

struct Header {
    pub file_size: u32,
    // magic_number: u16 = 0xA5E0
    pub frames_count: u16,
    pub width: u16,
    pub height: u16,

    /// Color depth in bits per pixel.
    pub color_depth: u16,

    /// Flags for all layers in the file.
    pub flags: HeaderFlags,

    // speed: u16 = 0 (deprecated)
    // reserved: [0u32; 2]
    /// The index of the transparent color if mode is indexed.
    ///
    /// Otherwise unused, set to 0
    pub transparent_index: u8,

    // reserved: [0u8; 3] (fills out the dword alignment)
    /// Number of colors in the palette for indexed color mode.
    pub num_indexed_colors: u16,

    /// Pixel width (for non-square pixels). With [`pixel_height`], gives the pixel aspect ratio.
    ///
    /// If zero, pixels are square.
    pub pixel_width: u8,

    /// Pixel width (for non-square pixels). With [`pixel_height`], gives the pixel aspect ratio.
    ///
    /// If zero, pixels are square.
    pub pixel_height: u8,
    // Following are only used in the case of grids, which we shouldn't need to support.
    pub grid_x: i16,
    pub grid_y: i16,
    pub grid_width: u16,
    pub grid_height: u16,

    // padded to 128 bytes
    pub reserved2: [u8; 84],
}

impl Header {
    fn to_block(&self, factory: &BlockBuilderFactory) -> io::Result<Block> {
        let mut builder = factory.create();
        builder.write_u32_le(self.file_size)?;
        builder.write_u16_le(0xA5E0)?;
        builder.write_u16_le(self.frames_count)?;
        builder.write_u16_le(self.width)?;
        builder.write_u16_le(self.height)?;
        builder.write_u16_le(self.color_depth)?;
        builder.write_u32_le(self.flags.bits())?;
        builder.write_u16_le(0)?;
        builder.write_bytes(&[0u8; 8])?; // reserved
        builder.write_u8(self.transparent_index)?;
        builder.write_bytes(&[0u8; 3])?; // reserved
        builder.write_u16_le(self.num_indexed_colors)?;
        builder.write_u8(self.pixel_width)?;
        builder.write_u8(self.pixel_height)?;
        builder.write_i16_le(self.grid_x)?;
        builder.write_i16_le(self.grid_y)?;
        builder.write_u16_le(self.grid_width)?;
        builder.write_u16_le(self.grid_height)?;
        builder.write_bytes(&[0u8; 84])?;
        builder.build()
    }
}

#[derive(Debug, Clone)]
pub(super) struct FrameHeader {
    /// Frame data size in bytes, including this header.
    frame_size: u32,

    // magic_number: u16 = 0xF1FA

    // Old chunks count. Using the newer field instead.
    // old_chunks_count: u16 = 0xFFFF
    /// The duration if this frame in milliseconds.
    duration_ms: u16,

    // reserved: [0u8; 2],
    /// Number of chunks in this frame.
    num_chunks: u32,
}

impl FrameHeader {
    fn to_block(&self, factory: &BlockBuilderFactory) -> io::Result<Block> {
        let mut builder = factory.create();
        builder.write_u32_le(self.frame_size)?;
        builder.write_u16_le(0xF1FA)?;
        // Old chunks count. Using the newer field instead.
        let old_chunks = if self.num_chunks < 0xFFFF {
            u16::try_from(self.num_chunks).unwrap()
        } else {
            0xFFFF
        };
        builder.write_u16_le(old_chunks)?;
        builder.write_u16_le(self.duration_ms)?;
        builder.write_bytes(&[0u8; 2])?;
        builder.write_u32_le(if self.num_chunks < 0xFFFF {
            0
        } else {
            self.num_chunks
        })?;
        builder.build()
    }
}

fn build_frame_block(
    block_factory: &BlockBuilderFactory,
    duration_ms: u16,
    chunks: impl IntoIterator<Item = ChunkBlock>,
) -> io::Result<Block> {
    let chunk_blocks = chunks
        .into_iter()
        .map(|chunk| chunk.to_block(block_factory))
        .collect::<Result<Vec<_>, _>>()?;
    let num_chunks = u32::try_from(chunk_blocks.len()).unwrap();
    let frame_contents = block_factory.concat(chunk_blocks)?;
    let frame_header = FrameHeader {
        frame_size: u32::try_from(frame_contents.len() + 16).unwrap(),
        duration_ms,
        num_chunks,
    };
    let header_block = frame_header.to_block(block_factory)?;

    block_factory.concat([header_block, frame_contents])
}

#[derive(Debug, Clone)]
struct ChunkHeader {
    /// Size of this chunk in bytes, including this header.
    chunk_size: u32,

    /// The type of this chunk.
    chunk_type: u16,
}

impl ChunkHeader {
    fn to_block(&self, factory: &BlockBuilderFactory) -> io::Result<Block> {
        let mut builder = factory.create();
        builder.write_u32_le(self.chunk_size)?;
        builder.write_u16_le(self.chunk_type)?;
        builder.build()
    }
}

#[derive(Debug, Clone)]
struct ChunkBlock {
    data: Arc<dyn ChunkValue>,
}

impl ChunkBlock {
    fn from_value<V>(value: V) -> Self
    where
        V: ChunkValue + 'static,
    {
        Self {
            data: Arc::new(value),
        }
    }

    fn to_block(&self, block_factory: &BlockBuilderFactory) -> io::Result<Block> {
        let chunk_block = self.data.to_block(block_factory)?;
        let chunk_header = ChunkHeader {
            chunk_size: u32::try_from(chunk_block.len() + 6).unwrap(),
            chunk_type: self.data.chunk_type(),
        };
        let header_block = chunk_header.to_block(block_factory)?;

        block_factory.concat([header_block, chunk_block])
    }
}

/// Types that are
trait ChunkValue: std::fmt::Debug {
    fn chunk_type(&self) -> u16;
    fn to_block(&self, factory: &BlockBuilderFactory) -> io::Result<Block>;
}

trait ChunkType: ChunkValue + Sized {
    const CHUNK_TYPE: u16;

    fn to_block(&self, factory: &BlockBuilderFactory) -> io::Result<Block>;

    fn from_block<M>(block: M) -> MemResult<Self>
    where
        M: MemReader;
}

impl<T> ChunkValue for T
where
    T: ChunkType,
{
    fn chunk_type(&self) -> u16 {
        Self::CHUNK_TYPE
    }

    fn to_block(&self, factory: &BlockBuilderFactory) -> io::Result<Block> {
        <Self as ChunkType>::to_block(self, factory)
    }
}
mod layer {
    use scidev::utils::{
        block::{Block, BlockBuilderFactory},
        mem_reader::{MemReader, Result as MemResult},
    };
    use std::io;

    use crate::formats::aseprite::{
        BlendMode, LayerFlags, LayerType,
        backing::LayerContents,
        raw::{ExtensionContext, user_data::UserDataChunk},
    };

    use super::ChunkType;

    /// A chunk describing a layer.
    #[derive(Clone, Debug)]
    pub(super) struct LayerChunk {
        pub flags: LayerFlags,
        pub layer_type: LayerType,
        pub child_level: u16,
        pub default_width: u16,
        pub default_height: u16,
        pub blend_mode: BlendMode,
        pub opacity: u8,
        // padding: [0u8; 3]
        pub layer_name: String,
        pub uuid: Option<[u8; 16]>,
    }

    impl ChunkType for LayerChunk {
        const CHUNK_TYPE: u16 = 0x2004;

        fn to_block(&self, factory: &BlockBuilderFactory) -> io::Result<Block> {
            let mut builder = factory.create();
            builder.write_u16_le(self.flags.bits())?;
            builder.write_u16_le(match &self.layer_type {
                LayerType::Normal => 0,
                LayerType::Group => 1,
                LayerType::Tilemap { .. } => 2,
            })?;
            builder.write_u16_le(self.child_level)?;
            builder.write_u16_le(0)?; // default_width
            builder.write_u16_le(0)?; // default_height
            builder.write_u16_le(self.blend_mode as u16)?;
            builder.write_u8(self.opacity)?;
            builder.write_bytes(&[0u8; 3])?;
            let name_len = u16::try_from(self.layer_name.len()).unwrap_or(u16::MAX);
            builder.write_u16_le(name_len)?;
            builder.write_bytes(self.layer_name.as_bytes())?;
            if let LayerType::Tilemap { tileset_index } = &self.layer_type {
                builder.write_u32_le(*tileset_index)?;
            }
            if let Some(uuid) = &self.uuid {
                builder.write_bytes(uuid)?;
            }
            builder.build()
        }

        fn from_block<M>(mut reader: M) -> MemResult<Self>
        where
            M: MemReader,
        {
            let flags = LayerFlags::from_bits_truncate(reader.read_u16_le()?);
            let layer_type_val = reader.read_u16_le()?;
            let child_level = reader.read_u16_le()?;
            let _default_width = reader.read_u16_le()?;
            let _default_height = reader.read_u16_le()?;
            let blend_mode_val = reader.read_u16_le()?;
            let opacity = reader.read_u8()?;
            let _padding = reader.read_values::<u8>("padding", 3)?;
            let name_len = reader.read_u16_le()?;
            let layer_name =
                String::from_utf8_lossy(&reader.read_values::<u8>("name", name_len as usize)?)
                    .to_string();

            let layer_type = match layer_type_val {
                0 => LayerType::Normal,
                1 => LayerType::Group,
                2 => {
                    let tileset_index = reader.read_u32_le()?;
                    LayerType::Tilemap { tileset_index }
                }
                _ => {
                    return Err(reader
                        .create_invalid_data_error_msg("Invalid layer type")
                        .into());
                }
            };

            let blend_mode = match blend_mode_val {
                0 => BlendMode::Normal,
                1 => BlendMode::Multiply,
                2 => BlendMode::Screen,
                3 => BlendMode::Overlay,
                4 => BlendMode::Darken,
                5 => BlendMode::Lighten,
                6 => BlendMode::ColorDodge,
                7 => BlendMode::ColorBurn,
                8 => BlendMode::HardLight,
                9 => BlendMode::SoftLight,
                10 => BlendMode::Difference,
                11 => BlendMode::Exclusion,
                12 => BlendMode::Hue,
                13 => BlendMode::Saturation,
                14 => BlendMode::Color,
                15 => BlendMode::Luminosity,
                16 => BlendMode::Addition,
                17 => BlendMode::Subtraction,
                18 => BlendMode::Divide,
                _ => {
                    return Err(reader
                        .create_invalid_data_error_msg("Invalid blend mode")
                        .into());
                }
            };

            let uuid = if reader.remaining() >= 16 {
                let mut uuid_bytes = [0u8; 16];
                let bytes = reader.read_values::<u8>("uuid", 16)?;
                uuid_bytes.copy_from_slice(&bytes);
                Some(uuid_bytes)
            } else {
                None
            };

            Ok(LayerChunk {
                flags,
                layer_type,
                child_level,
                blend_mode,
                opacity,
                layer_name,
                uuid,
                default_width: 0,
                default_height: 0,
            })
        }
    }

    impl LayerChunk {
        pub(super) fn from_backing(
            ext_context: &ExtensionContext,
            layers: &[LayerContents],
        ) -> Vec<super::ChunkBlock> {
            let mut chunks = Vec::new();
            for layer in layers {
                chunks.push(super::ChunkBlock::from_value(LayerChunk {
                    flags: layer.flags,
                    layer_type: layer.layer_type,
                    child_level: 0,
                    default_width: 0,
                    default_height: 0,
                    blend_mode: layer.blend_mode,
                    opacity: layer.opacity,
                    layer_name: layer.name.clone(),
                    uuid: layer.uuid,
                }));

                if let Some(chunk) = UserDataChunk::from_backing(ext_context, &layer.user_data)
                    .map(super::ChunkBlock::from_value)
                {
                    chunks.push(chunk);
                }
            }
            chunks
        }
    }
}

mod cel {
    use scidev::utils::{
        block::{Block, BlockBuilder, BlockBuilderFactory},
        mem_reader::{MemReader, Result as MemResult},
    };
    use std::io;

    use crate::formats::aseprite::{
        CelIndex,
        backing::{CelContents, CelData},
        raw::{ExtensionContext, user_data::UserDataChunk},
    };

    use super::ChunkType;

    /// A raw pixel cel (uncompressed).
    #[derive(Clone, Debug)]
    pub(super) struct RawCel {
        width: u16,
        height: u16,
        pixels: Vec<u8>,
    }

    impl RawCel {
        fn write(&self, builder: &mut BlockBuilder) -> io::Result<()> {
            builder.write_u16_le(self.width)?;
            builder.write_u16_le(self.height)?;
            builder.write_bytes(&self.pixels)?;
            Ok(())
        }

        fn read<M: MemReader>(reader: &mut M) -> MemResult<Self> {
            let width = reader.read_u16_le()?;
            let height = reader.read_u16_le()?;
            let pixels = reader.read_remaining()?;
            Ok(Self {
                width,
                height,
                pixels,
            })
        }
    }

    /// A cel that links to another frame.
    #[derive(Clone, Debug)]
    pub(super) struct LinkedCel {
        pub frame_position: u16,
    }

    impl LinkedCel {
        fn write(&self, builder: &mut BlockBuilder) -> io::Result<()> {
            builder.write_u16_le(self.frame_position)?;
            Ok(())
        }

        fn read<M: MemReader>(reader: &mut M) -> MemResult<Self> {
            let frame_position = reader.read_u16_le()?;
            Ok(Self { frame_position })
        }
    }

    /// A compressed pixel cel.
    #[derive(Clone, Debug)]
    pub(super) struct CompressedCel {
        pub width: u16,
        pub height: u16,
        pub data: Vec<u8>,
    }

    impl CompressedCel {
        fn write(&self, builder: &mut BlockBuilder) -> io::Result<()> {
            builder.write_u16_le(self.width)?;
            builder.write_u16_le(self.height)?;
            builder.write_bytes(&self.data)?;
            Ok(())
        }

        fn read<M: MemReader>(reader: &mut M) -> MemResult<Self> {
            let width = reader.read_u16_le()?;
            let height = reader.read_u16_le()?;
            let data = reader.read_remaining()?;
            Ok(Self {
                width,
                height,
                data,
            })
        }
    }

    /// A compressed tilemap cel.
    #[derive(Clone, Debug)]
    pub(super) struct CompressedTilemapCel {
        pub width: u16,
        pub height: u16,
        pub bits_per_tile: u16,
        pub tile_id_bitmask: u32,
        pub x_flip_bitmask: u32,
        pub y_flip_bitmask: u32,
        pub diagonal_flip_bitmask: u32,
        pub tiles: Vec<u8>,
    }

    impl CompressedTilemapCel {
        fn write(&self, builder: &mut BlockBuilder) -> io::Result<()> {
            builder.write_u16_le(self.width)?;
            builder.write_u16_le(self.height)?;
            builder.write_u16_le(self.bits_per_tile)?;
            builder.write_u32_le(self.tile_id_bitmask)?;
            builder.write_u32_le(self.x_flip_bitmask)?;
            builder.write_u32_le(self.y_flip_bitmask)?;
            builder.write_u32_le(self.diagonal_flip_bitmask)?;
            builder.write_bytes(&[0; 10])?;
            builder.write_bytes(&self.tiles)?;
            Ok(())
        }

        fn read<M: MemReader>(reader: &mut M) -> MemResult<Self> {
            let width = reader.read_u16_le()?;
            let height = reader.read_u16_le()?;
            let bits_per_tile = reader.read_u16_le()?;
            let bitmask_tile_id = reader.read_u32_le()?;
            let x_flip_bitmask = reader.read_u32_le()?;
            let y_flip_bitmask = reader.read_u32_le()?;
            let bitmask_diagonal_flip = reader.read_u32_le()?;
            let _reserved = reader.read_values::<u8>("reserved", 10)?;
            let tiles = reader.read_remaining()?;
            Ok(Self {
                width,
                height,
                bits_per_tile,
                tile_id_bitmask: bitmask_tile_id,
                x_flip_bitmask,
                y_flip_bitmask,
                diagonal_flip_bitmask: bitmask_diagonal_flip,
                tiles,
            })
        }
    }

    /// The specific data within a cel chunk.
    #[derive(Clone, Debug)]
    pub(super) enum CelType {
        Raw(RawCel),
        Linked(LinkedCel),
        Compressed(CompressedCel),
        CompressedTilemap(CompressedTilemapCel),
    }

    /// A chunk describing a cel.
    #[derive(Clone, Debug)]
    pub(super) struct CelChunk {
        layer_index: u16,
        x: i16,
        y: i16,
        opacity: u8,
        cel_type: CelType,
        z_index: i16,
        reserved: [u8; 5],
    }

    impl ChunkType for CelChunk {
        const CHUNK_TYPE: u16 = 0x2005;

        fn to_block(&self, factory: &BlockBuilderFactory) -> io::Result<Block> {
            let mut builder = factory.create();
            builder.write_u16_le(self.layer_index)?;
            builder.write_i16_le(self.x)?;
            builder.write_i16_le(self.y)?;
            builder.write_u8(self.opacity)?;
            let type_val = match &self.cel_type {
                CelType::Raw(_) => 0,
                CelType::Linked(_) => 1,
                CelType::Compressed(_) => 2,
                CelType::CompressedTilemap(_) => 3,
            };
            builder.write_u16_le(type_val)?;
            builder.write_i16_le(self.z_index)?;
            builder.write_bytes(&[0; 5])?;

            match &self.cel_type {
                CelType::Raw(cel) => cel.write(&mut builder)?,
                CelType::Linked(cel) => cel.write(&mut builder)?,
                CelType::Compressed(cel) => cel.write(&mut builder)?,
                CelType::CompressedTilemap(cel) => cel.write(&mut builder)?,
            }
            builder.build()
        }

        fn from_block<M>(mut reader: M) -> MemResult<Self>
        where
            M: MemReader,
        {
            let layer_index = reader.read_u16_le()?;
            let x = reader.read_i16_le()?;
            let y = reader.read_i16_le()?;
            let opacity = reader.read_u8()?;
            let cel_type_val = reader.read_u16_le()?;
            let z_index = reader.read_i16_le()?;
            let _reserved = reader.read_values::<u8>("reserved", 5)?;

            let cel_type = match cel_type_val {
                0 => CelType::Raw(RawCel::read(&mut reader)?),
                1 => CelType::Linked(LinkedCel::read(&mut reader)?),
                2 => CelType::Compressed(CompressedCel::read(&mut reader)?),
                3 => CelType::CompressedTilemap(CompressedTilemapCel::read(&mut reader)?),
                _ => {
                    return Err(reader
                        .create_invalid_data_error_msg("Invalid cel type")
                        .into());
                }
            };

            Ok(CelChunk {
                layer_index,
                x,
                y,
                opacity,
                cel_type,
                z_index,
                reserved: [0; 5],
            })
        }
    }

    impl CelChunk {
        pub(super) fn create_chunks(
            ext_context: &ExtensionContext,
            cels: &std::collections::BTreeMap<CelIndex, CelContents>,
            frame_idx: usize,
        ) -> io::Result<Vec<super::ChunkBlock>> {
            let mut chunks = Vec::new();

            for (index, cel) in cels {
                if index.frame as usize == frame_idx {
                    let chunk = match &cel.contents {
                        CelData::Pixels(pixels) => {
                            use std::io::Read;
                            let mut data =
                                Vec::with_capacity(usize::try_from(pixels.data.len()).unwrap());
                            // Handle open_reader error properly
                            pixels
                                .data
                                .open_reader(..)
                                .map_err(|e| io::Error::other(e.to_string()))?
                                .read_to_end(&mut data)?;

                            CelChunk {
                                layer_index: index.layer,
                                x: cel.position.x,
                                y: cel.position.y,
                                opacity: cel.opacity,
                                cel_type: CelType::Raw(RawCel {
                                    width: pixels.width,
                                    height: pixels.height,
                                    pixels: data,
                                }),
                                z_index: 0,
                                reserved: [0; 5],
                            }
                        }
                        CelData::Linked(linked_frame_idx) => CelChunk {
                            layer_index: index.layer,
                            x: cel.position.x,
                            y: cel.position.y,
                            opacity: cel.opacity,
                            cel_type: CelType::Linked(LinkedCel {
                                frame_position: *linked_frame_idx,
                            }),
                            z_index: 0,
                            reserved: [0; 5],
                        },
                        CelData::Tilemap => continue,
                    };
                    chunks.push(super::ChunkBlock::from_value(chunk));

                    if let Some(chunk) = UserDataChunk::from_backing(ext_context, &cel.user_data)
                        .map(super::ChunkBlock::from_value)
                    {
                        chunks.push(chunk);
                    }
                }
            }
            Ok(chunks)
        }
    }
}

mod cel_extra {
    use scidev::utils::{
        block::{Block, BlockBuilderFactory},
        mem_reader::{MemReader, Result as MemResult},
    };
    use std::io;

    use super::ChunkType;

    /// Extra chunk for precise cel positioning.
    #[derive(Clone, Debug)]
    pub(super) struct CelExtraChunk {
        flags: u32,     // 1 = precise bounds are set
        precise_x: i32, // FIXED 16.16
        precise_y: i32, // FIXED 16.16
        width: i32,     // FIXED 16.16
        height: i32,    // FIXED 16.16
    }

    impl ChunkType for CelExtraChunk {
        const CHUNK_TYPE: u16 = 0x2006;

        fn to_block(&self, factory: &BlockBuilderFactory) -> io::Result<Block> {
            let mut builder = factory.create();
            builder.write_u32_le(self.flags)?;
            builder.write_i32_le(self.precise_x)?;
            builder.write_i32_le(self.precise_y)?;
            builder.write_i32_le(self.width)?;
            builder.write_i32_le(self.height)?;
            builder.write_bytes(&[0; 16])?;
            builder.build()
        }

        fn from_block<M>(mut reader: M) -> MemResult<Self>
        where
            M: MemReader,
        {
            let flags = reader.read_u32_le()?;
            let precise_x = reader.read_value::<i32>("precise_x")?;
            let precise_y = reader.read_value::<i32>("precise_y")?;
            let width = reader.read_value::<i32>("width")?;
            let height = reader.read_value::<i32>("height")?;
            let _reserved = reader.read_values::<u8>("reserved", 16)?;

            Ok(CelExtraChunk {
                flags,
                precise_x,
                precise_y,
                width,
                height,
            })
        }
    }
}

mod tags {
    use scidev::utils::{
        block::{Block, BlockBuilderFactory},
        mem_reader::{MemReader, Result as MemResult},
    };
    use std::io;

    use crate::formats::aseprite::raw::ExtensionContext;

    use super::ChunkType;

    /// Direction of animation for a tag.
    #[derive(Clone, Copy, Debug)]
    #[repr(u8)]
    pub(super) enum AnimationDirection {
        Forward = 0,
        Reverse = 1,
        PingPong = 2,
        PingPongReverse = 3,
    }

    /// A single animation tag.
    #[derive(Clone, Debug)]
    pub(super) struct Tag {
        from_frame: u16,
        to_frame: u16,
        direction: AnimationDirection,
        repeat: u16,
        // tag color is deprecated, used only for backward compatibility
        // tag name
        name: String,
    }

    impl Tag {
        pub(super) fn new(
            from_frame: u16,
            to_frame: u16,
            direction: AnimationDirection,
            repeat: u16,
            name: String,
        ) -> Self {
            Self {
                from_frame,
                to_frame,
                direction,
                repeat,
                name,
            }
        }
    }

    /// A chunk containing a list of tags.
    #[derive(Clone, Debug)]
    pub(super) struct TagsChunk {
        tags: Vec<Tag>,
    }

    impl TagsChunk {
        pub(super) fn new(tags: Vec<Tag>) -> Self {
            Self { tags }
        }
    }

    impl ChunkType for TagsChunk {
        const CHUNK_TYPE: u16 = 0x2018;

        fn to_block(&self, factory: &BlockBuilderFactory) -> io::Result<Block> {
            let mut builder = factory.create();
            builder.write_u16_le(self.tags.len().try_into().unwrap())?;
            builder.write_bytes(&[0; 8])?;
            for tag in &self.tags {
                builder.write_u16_le(tag.from_frame)?;
                builder.write_u16_le(tag.to_frame)?;
                builder.write_u8(tag.direction as u8)?;
                builder.write_u16_le(tag.repeat)?;
                builder.write_bytes(&[0; 6])?;
                builder.write_bytes(&[0; 3])?; // deprecated color
                builder.write_u8(0)?; // extra byte
                let name_len = u16::try_from(tag.name.len()).unwrap_or(u16::MAX);
                builder.write_u16_le(name_len)?;
                builder.write_bytes(tag.name.as_bytes())?;
            }
            builder.build()
        }

        fn from_block<M>(mut reader: M) -> MemResult<Self>
        where
            M: MemReader,
        {
            let num_tags = reader.read_u16_le()?;
            let _reserved = reader.read_values::<u8>("reserved", 8)?;
            let mut tags = Vec::with_capacity(num_tags as usize);

            for _ in 0..num_tags {
                let from_frame = reader.read_u16_le()?;
                let to_frame = reader.read_u16_le()?;
                let direction_val = reader.read_u8()?;
                let repeat = reader.read_u16_le()?;
                let _reserved_tag = reader.read_values::<u8>("reserved_tag", 6)?;
                let _deprecated_color = reader.read_values::<u8>("deprecated_color", 3)?;
                let _extra = reader.read_u8()?;
                let name_len = reader.read_u16_le()?;
                let name =
                    String::from_utf8_lossy(&reader.read_values::<u8>("name", name_len as usize)?)
                        .to_string();

                let direction = match direction_val {
                    0 => AnimationDirection::Forward,
                    1 => AnimationDirection::Reverse,
                    2 => AnimationDirection::PingPong,
                    3 => AnimationDirection::PingPongReverse,
                    _ => {
                        return Err(reader
                            .create_invalid_data_error_msg("Invalid animation direction")
                            .into());
                    }
                };

                tags.push(Tag {
                    from_frame,
                    to_frame,
                    direction,
                    repeat,
                    name,
                });
            }

            Ok(TagsChunk { tags })
        }
    }

    impl TagsChunk {
        pub(super) fn from_backing(
            ext_context: &ExtensionContext,
            tags: &[crate::formats::aseprite::backing::TagContents],
        ) -> Vec<super::ChunkBlock> {
            use crate::formats::aseprite::AnimationDirection as BackingDir;
            use crate::formats::aseprite::raw::user_data::UserDataChunk;

            let mut chunks = Vec::new();
            if tags.is_empty() {
                return chunks;
            }

            let tags_vec: Vec<Tag> = tags
                .iter()
                .map(|t| {
                    let direction = match t.direction {
                        BackingDir::Forward => AnimationDirection::Forward,
                        BackingDir::Backward => AnimationDirection::Reverse,
                        BackingDir::PingPong => AnimationDirection::PingPong,
                        BackingDir::PingPongReverse => AnimationDirection::PingPongReverse,
                    };

                    Tag {
                        from_frame: u16::try_from(t.from_frame).unwrap_or(0),
                        to_frame: u16::try_from(t.to_frame).unwrap_or(0),
                        direction,
                        repeat: 0,
                        name: t.name.clone(),
                    }
                })
                .collect();

            chunks.push(super::ChunkBlock::from_value(TagsChunk { tags: tags_vec }));

            // Strict 1:1 Tag User Data
            for t in tags {
                let ud_chunk = UserDataChunk::from_backing(ext_context, &t.user_data)
                    .unwrap_or_else(UserDataChunk::empty);
                chunks.push(super::ChunkBlock::from_value(ud_chunk));
            }

            chunks
        }
    }
}

mod palette {
    use bitflags::bitflags;
    use scidev::utils::{
        block::{Block, BlockBuilderFactory},
        mem_reader::{MemReader, Result as MemResult},
    };
    use std::io;

    use super::ChunkType;

    bitflags! {
        /// Flags for a palette entry.
        #[derive(Clone, Debug)]
        pub struct PaletteEntryFlags: u16 {
            /// The entry has a name.
            const HAS_NAME = 0x0001;
        }
    }

    /// A single entry in the palette.
    #[derive(Clone, Debug)]
    pub(super) struct PaletteEntry {
        flags: PaletteEntryFlags,
        red: u8,
        green: u8,
        blue: u8,
        alpha: u8,
        name: Option<String>,
    }

    /// A chunk describing the palette.
    #[derive(Clone, Debug)]
    pub(super) struct PaletteChunk {
        new_palette_size: u32,
        first_color_index: u32,
        last_color_index: u32,
        entries: Vec<PaletteEntry>,
    }

    impl ChunkType for PaletteChunk {
        const CHUNK_TYPE: u16 = 0x2019;

        fn to_block(&self, factory: &BlockBuilderFactory) -> io::Result<Block> {
            let mut builder = factory.create();
            builder.write_u32_le(self.new_palette_size)?;
            builder.write_u32_le(self.first_color_index)?;
            builder.write_u32_le(self.last_color_index)?;
            builder.write_bytes(&[0; 8])?;
            for entry in &self.entries {
                builder.write_u16_le(entry.flags.bits())?;
                builder.write_u8(entry.red)?;
                builder.write_u8(entry.green)?;
                builder.write_u8(entry.blue)?;
                builder.write_u8(entry.alpha)?;
                if entry.flags.contains(PaletteEntryFlags::HAS_NAME) {
                    if let Some(name) = &entry.name {
                        let name_len = u16::try_from(name.len()).unwrap_or(u16::MAX);
                        builder.write_u16_le(name_len)?;
                        builder.write_bytes(name.as_bytes())?;
                    } else {
                        builder.write_u16_le(0)?;
                    }
                }
            }
            builder.build()
        }

        fn from_block<M>(mut reader: M) -> MemResult<Self>
        where
            M: MemReader,
        {
            let new_palette_size = reader.read_u32_le()?;
            let first_color_index = reader.read_u32_le()?;
            let last_color_index = reader.read_u32_le()?;
            let _reserved = reader.read_values::<u8>("reserved", 8)?;

            let mut entries = Vec::with_capacity(new_palette_size as usize);
            for _ in 0..new_palette_size {
                let flags_val = reader.read_u16_le()?;
                let flags = PaletteEntryFlags::from_bits_truncate(flags_val);
                let red = reader.read_u8()?;
                let green = reader.read_u8()?;
                let blue = reader.read_u8()?;
                let alpha = reader.read_u8()?;
                let name = if flags.contains(PaletteEntryFlags::HAS_NAME) {
                    let name_len = reader.read_u16_le()?;
                    Some(
                        String::from_utf8_lossy(
                            &reader.read_values::<u8>("name", name_len as usize)?,
                        )
                        .to_string(),
                    )
                } else {
                    None
                };
                entries.push(PaletteEntry {
                    flags,
                    red,
                    green,
                    blue,
                    alpha,
                    name,
                });
            }

            Ok(PaletteChunk {
                new_palette_size,
                first_color_index,
                last_color_index,
                entries,
            })
        }
    }

    impl PaletteChunk {
        pub(super) fn from_backing(
            palette: &crate::formats::aseprite::backing::PaletteContents,
        ) -> Option<super::ChunkBlock> {
            if palette.entries.is_empty() {
                return None;
            }

            let entries = palette
                .entries
                .iter()
                .map(|e| {
                    let mut flags = PaletteEntryFlags::empty();
                    if e.name().is_some() {
                        flags |= PaletteEntryFlags::HAS_NAME;
                    }
                    PaletteEntry {
                        flags,
                        red: e.color().red(),
                        green: e.color().green(),
                        blue: e.color().blue(),
                        alpha: e.color().alpha(),
                        name: e.name().map(ToString::to_string),
                    }
                })
                .collect();

            Some(super::ChunkBlock::from_value(PaletteChunk {
                new_palette_size: u32::try_from(palette.entries.len()).unwrap_or(0),
                first_color_index: 0,
                last_color_index: u32::try_from(palette.entries.len().saturating_sub(1))
                    .unwrap_or(0),
                entries,
            }))
        }
    }
}

pub(super) mod user_data {
    use bitflags::bitflags;

    use scidev::utils::{
        block::{Block, BlockBuilderFactory},
        mem_reader::{MemReader, Result as MemResult},
    };
    use std::{collections::BTreeMap, io, num::NonZeroU32};

    use crate::formats::aseprite::{
        Properties,
        backing::{self, UserData},
        raw::{ExtensionContext, UserDataPropsKey},
    };

    use super::ChunkType;

    bitflags! {
        /// Flags for user data.
        #[derive(Clone, Debug)]
        pub struct UserDataFlags: u32 {
            /// User data has text.
            const HAS_TEXT = 0x0001;
            /// User data has color.
            const HAS_COLOR = 0x0002;
            /// User data has properties.
            const HAS_PROPERTIES = 0x0004;
        }
    }

    /// A chunk containing user data.
    #[derive(Clone, Debug)]
    pub(super) struct UserDataChunk {
        flags: UserDataFlags,
        text: Option<String>,
        color: Option<[u8; 4]>, // RGBA
        properties_data: BTreeMap<UserDataPropsKey, Properties>,
    }

    impl UserDataChunk {
        pub(super) fn from_backing(
            ext_context: &ExtensionContext,
            user_data: &UserData,
        ) -> Option<Self> {
            let mut flags = UserDataFlags::empty();
            let mut has_content = false;

            let text = if let Some(t) = &user_data.text {
                flags |= UserDataFlags::HAS_TEXT;
                has_content = true;
                Some(t.clone())
            } else {
                None
            };

            let color = if let Some(c) = &user_data.color {
                flags |= UserDataFlags::HAS_COLOR;
                has_content = true;
                Some([c.red(), c.green(), c.blue(), c.alpha()])
            } else {
                None
            };

            let mut properties_data = BTreeMap::new();
            if !user_data.properties.is_empty() {
                flags |= UserDataFlags::HAS_PROPERTIES;
                has_content = true;
                for (key, props) in &user_data.properties {
                    let new_key = match key {
                        backing::UserDataPropsKey::General => UserDataPropsKey::General,
                        backing::UserDataPropsKey::Extension(name) => {
                            let Some(id) = ext_context.name_mapping.get(name) else {
                                panic!("Extension {name} not found in context");
                            };
                            UserDataPropsKey::ForExtension(NonZeroU32::new(id.to_u32()).unwrap())
                        }
                    };
                    properties_data.insert(new_key, props.clone());
                }
            }

            if has_content {
                Some(Self {
                    flags,
                    text,
                    color,
                    properties_data,
                })
            } else {
                None
            }
        }

        pub(super) fn empty() -> Self {
            Self {
                flags: UserDataFlags::empty(),
                text: None,
                color: None,
                properties_data: BTreeMap::new(),
            }
        }
    }

    impl ChunkType for UserDataChunk {
        const CHUNK_TYPE: u16 = 0x2020;

        fn to_block(&self, factory: &BlockBuilderFactory) -> io::Result<Block> {
            let mut builder = factory.create();
            builder.write_u32_le(self.flags.bits())?;
            if let Some(text) = &self.text {
                builder.write_u16_le(text.len().try_into().unwrap())?;
                builder.write_bytes(text.as_bytes())?;
            }
            if let Some(color) = self.color {
                builder.write_bytes(&color)?;
            }
            if !self.properties_data.is_empty() {
                let mut props_builder = factory.create();
                let num_maps = u32::try_from(self.properties_data.len()).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "Too many property maps")
                })?;
                props_builder.write_u32_le(num_maps)?;

                for (key, props) in &self.properties_data {
                    let key_val = match key {
                        UserDataPropsKey::General => 0,
                        UserDataPropsKey::ForExtension(id) => id.get(),
                    };
                    props_builder.write_u32_le(key_val)?;
                    props.write_to(&mut props_builder)?;
                }

                let props_block = props_builder.build()?;
                // Size includes the 4 bytes for the count and 4 bytes for the size field itself.
                // props_block contains count + maps.
                builder.write_u32_le(u32::try_from(props_block.len()).unwrap() + 4)?;
                builder.write_block(&props_block)?;
            }
            builder.build()
        }

        fn from_block<M>(mut reader: M) -> MemResult<Self>
        where
            M: MemReader,
        {
            let flags_val = reader.read_u32_le()?;
            let flags = UserDataFlags::from_bits_truncate(flags_val);

            let text = if flags.contains(UserDataFlags::HAS_TEXT) {
                let len = reader.read_u16_le()?;
                Some(
                    String::from_utf8_lossy(&reader.read_values::<u8>("text", len as usize)?)
                        .to_string(),
                )
            } else {
                None
            };

            let color = if flags.contains(UserDataFlags::HAS_COLOR) {
                let r = reader.read_u8()?;
                let g = reader.read_u8()?;
                let b = reader.read_u8()?;
                let a = reader.read_u8()?;
                Some([r, g, b, a])
            } else {
                None
            };

            let properties_data = if flags.contains(UserDataFlags::HAS_PROPERTIES) {
                let size = reader.read_u32_le()?;
                // Sanity check size. Spec says >= 8 bytes (4 for size + 4 for count)
                if size < 8 {
                    return Err(reader
                        .create_invalid_data_error_msg("Invalid properties size")
                        .into());
                }
                let count = reader.read_u32_le()?;
                let mut map = BTreeMap::new();
                for _ in 0..count {
                    let key_val = reader.read_u32_le()?;
                    let key = if key_val == 0 {
                        UserDataPropsKey::General
                    } else {
                        UserDataPropsKey::ForExtension(NonZeroU32::new(key_val).ok_or_else(
                            || reader.create_invalid_data_error_msg("Invalid extension ID 0"),
                        )?)
                    };
                    let props = Properties::read_from(&mut reader)?;
                    map.insert(key, props);
                }
                map
            } else {
                BTreeMap::new()
            };

            Ok(UserDataChunk {
                flags,
                text,
                color,
                properties_data,
            })
        }
    }
}

mod slice {
    use bitflags::bitflags;
    use scidev::utils::{
        block::{Block, BlockBuilderFactory},
        mem_reader::{MemReader, Result as MemResult},
    };
    use std::io;

    use super::ChunkType;

    bitflags! {
        /// Flags for slice keys.
        #[derive(Clone, Debug)]
        pub struct SliceFlags: u32 {
            /// It's a 9-patches slice.
            const IS_9_PATCHES = 0x0001;
            /// Has pivot information.
            const HAS_PIVOT = 0x0002;
        }
    }

    /// A keyframe for a slice.
    #[derive(Clone, Debug)]
    pub(super) struct SliceKey {
        frame_number: u32,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        center: Option<(i32, i32, u32, u32)>, // x, y, w, h
        pivot: Option<(i32, i32)>,            // x, y
    }

    /// A chunk describing a slice.
    #[derive(Clone, Debug)]
    pub(super) struct SliceChunk {
        num_slice_keys: u32,
        flags: SliceFlags,
        name: String,
        keys: Vec<SliceKey>,
    }

    impl ChunkType for SliceChunk {
        const CHUNK_TYPE: u16 = 0x2022;

        fn to_block(&self, factory: &BlockBuilderFactory) -> io::Result<Block> {
            let mut builder = factory.create();
            builder.write_u32_le(self.keys.len().try_into().unwrap())?;
            builder.write_u32_le(self.flags.bits())?;
            builder.write_u32_le(0)?; // reserved
            builder.write_u16_le(self.name.len().try_into().unwrap())?;
            builder.write_bytes(self.name.as_bytes())?;

            for key in &self.keys {
                builder.write_u32_le(key.frame_number)?;
                builder.write_i32_le(key.x)?;
                builder.write_i32_le(key.y)?;
                builder.write_u32_le(key.width)?;
                builder.write_u32_le(key.height)?;
                if let Some((cx, cy, cw, ch)) = key.center {
                    builder.write_i32_le(cx)?;
                    builder.write_i32_le(cy)?;
                    builder.write_u32_le(cw)?;
                    builder.write_u32_le(ch)?;
                }
                if let Some((px, py)) = key.pivot {
                    builder.write_i32_le(px)?;
                    builder.write_i32_le(py)?;
                }
            }
            builder.build()
        }

        fn from_block<M>(mut reader: M) -> MemResult<Self>
        where
            M: MemReader,
        {
            let num_keys = reader.read_u32_le()?;
            let flags_val = reader.read_u32_le()?;
            let flags = SliceFlags::from_bits_truncate(flags_val);
            let _reserved = reader.read_u32_le()?;
            let name_len = reader.read_u16_le()?;
            let name =
                String::from_utf8_lossy(&reader.read_values::<u8>("name", name_len as usize)?)
                    .to_string();

            let mut keys = Vec::with_capacity(num_keys as usize);
            for _ in 0..num_keys {
                let frame_number = reader.read_u32_le()?;
                let x = reader.read_value::<i32>("x")?;
                let y = reader.read_value::<i32>("y")?;
                let width = reader.read_u32_le()?;
                let height = reader.read_u32_le()?;

                let center = if flags.contains(SliceFlags::IS_9_PATCHES) {
                    let cx = reader.read_value::<i32>("cx")?;
                    let cy = reader.read_value::<i32>("cy")?;
                    let cw = reader.read_u32_le()?;
                    let ch = reader.read_u32_le()?;
                    Some((cx, cy, cw, ch))
                } else {
                    None
                };

                let pivot = if flags.contains(SliceFlags::HAS_PIVOT) {
                    let px = reader.read_value::<i32>("px")?;
                    let py = reader.read_value::<i32>("py")?;
                    Some((px, py))
                } else {
                    None
                };

                keys.push(SliceKey {
                    frame_number,
                    x,
                    y,
                    width,
                    height,
                    center,
                    pivot,
                });
            }

            Ok(SliceChunk {
                num_slice_keys: num_keys,
                flags,
                name,
                keys,
            })
        }
    }
}

mod tileset {
    use bitflags::bitflags;
    use scidev::utils::{
        block::{Block, BlockBuilderFactory},
        mem_reader::{MemReader, Result as MemResult},
    };
    use std::io;

    use super::ChunkType;

    bitflags! {
        /// Flags for a tileset.
        #[derive(Clone, Debug)]
        pub struct TilesetFlags: u32 {
            /// Includes link to external file.
            const EXTERNAL_FILE = 0x0001;
            /// Includes tiles inside this file.
            const EMBEDDED = 0x0002;
            /// Tile 0 is empty.
            const ZERO_IS_EMPTY = 0x0004;
            /// Match X-flip.
            const MATCH_X_FLIP = 0x0008;
            /// Match Y-flip.
            const MATCH_Y_FLIP = 0x0010;
            /// Match Diagonal-flip.
            const MATCH_D_FLIP = 0x0020;
        }
    }

    /// A chunk describing a tileset.
    #[derive(Clone, Debug)]
    pub(super) struct TilesetChunk {
        id: u32,
        flags: TilesetFlags,
        num_tiles: u32,
        tile_width: u16,
        tile_height: u16,
        base_index: i16,
        name: String,
        external_file_id: Option<u32>,
        external_tileset_id: Option<u32>,
        compressed_data: Option<Vec<u8>>,
    }

    impl ChunkType for TilesetChunk {
        const CHUNK_TYPE: u16 = 0x2023;

        fn to_block(&self, factory: &BlockBuilderFactory) -> io::Result<Block> {
            let mut builder = factory.create();
            builder.write_u32_le(self.id)?;
            builder.write_u32_le(self.flags.bits())?;
            builder.write_u32_le(self.num_tiles)?;
            builder.write_u16_le(self.tile_width)?;
            builder.write_u16_le(self.tile_height)?;
            builder.write_i16_le(self.base_index)?;
            builder.write_bytes(&[0; 14])?;
            builder.write_u16_le(self.name.len().try_into().unwrap())?;
            builder.write_bytes(self.name.as_bytes())?;
            if self.flags.contains(TilesetFlags::EXTERNAL_FILE) {
                builder.write_u32_le(self.external_file_id.unwrap_or(0))?;
                builder.write_u32_le(self.external_tileset_id.unwrap_or(0))?;
            }
            if self.flags.contains(TilesetFlags::EMBEDDED) {
                if let Some(compressed) = &self.compressed_data {
                    builder.write_u32_le(compressed.len().try_into().unwrap())?;
                    builder.write_bytes(compressed)?;
                } else {
                    builder.write_u32_le(0)?;
                }
            }
            builder.build()
        }

        fn from_block<M>(mut reader: M) -> MemResult<Self>
        where
            M: MemReader,
        {
            let id = reader.read_u32_le()?;
            let flags_val = reader.read_u32_le()?;
            let flags = TilesetFlags::from_bits_truncate(flags_val);
            let num_tiles = reader.read_u32_le()?;
            let tile_width = reader.read_u16_le()?;
            let tile_height = reader.read_u16_le()?;
            let base_index = reader.read_i16_le()?;
            let _reserved = reader.read_values::<u8>("reserved", 14)?;
            let name_len = reader.read_u16_le()?;
            let name =
                String::from_utf8_lossy(&reader.read_values::<u8>("name", name_len as usize)?)
                    .to_string();

            let (external_file_id, external_tileset_id) =
                if flags.contains(TilesetFlags::EXTERNAL_FILE) {
                    (Some(reader.read_u32_le()?), Some(reader.read_u32_le()?))
                } else {
                    (None, None)
                };

            let compressed_data = if flags.contains(TilesetFlags::EMBEDDED) {
                let len = reader.read_u32_le()?;
                Some(reader.read_values::<u8>("compressed_data", len as usize)?)
            } else {
                None
            };

            Ok(TilesetChunk {
                id,
                flags,
                num_tiles,
                tile_width,
                tile_height,
                base_index,
                name,
                external_file_id,
                external_tileset_id,
                compressed_data,
            })
        }
    }
}

mod color_profile {
    use bitflags::bitflags;
    use scidev::utils::{
        block::{Block, BlockBuilderFactory},
        mem_reader::{MemReader, Result as MemResult},
    };
    use std::io;

    use crate::formats::aseprite::backing::ColorProfile;

    use super::ChunkType;

    /// The type of color profile.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[repr(u16)]
    pub(super) enum ColorProfileType {
        None = 0,
        Srgb = 1,
        Icc = 2,
    }

    bitflags! {
        /// Flags for color profile.
        #[derive(Clone, Debug)]
        pub struct ColorProfileFlags: u16 {
            /// Use fixed gamma.
            const FIXED_GAMMA = 0x0001;
        }
    }

    /// A chunk describing the color profile.
    #[derive(Clone, Debug)]
    pub(super) struct ColorProfileChunk {
        pub profile_type: ColorProfileType,
        pub flags: ColorProfileFlags,
        pub fixed_gamma: u32, // FIXED 16.16
        pub icc_profile: Option<Vec<u8>>,
    }

    impl ColorProfileChunk {
        pub(super) fn from_backing(profile: &ColorProfile) -> Option<super::ChunkBlock> {
            match profile {
                ColorProfile::Srgb => Some(super::ChunkBlock::from_value(ColorProfileChunk {
                    profile_type: ColorProfileType::Srgb,
                    flags: ColorProfileFlags::empty(),
                    fixed_gamma: 0,
                    icc_profile: None,
                })),
                ColorProfile::Icc(data) => Some(super::ChunkBlock::from_value(ColorProfileChunk {
                    profile_type: ColorProfileType::Icc,
                    flags: ColorProfileFlags::empty(),
                    fixed_gamma: 0,
                    icc_profile: Some(data.data.clone()),
                })),
                ColorProfile::None => None,
            }
        }
    }

    impl ChunkType for ColorProfileChunk {
        const CHUNK_TYPE: u16 = 0x2007;

        fn to_block(&self, factory: &BlockBuilderFactory) -> io::Result<Block> {
            let mut builder = factory.create();
            builder.write_u16_le(self.profile_type as u16)?;
            builder.write_u16_le(self.flags.bits())?;
            builder.write_u32_le(self.fixed_gamma)?;
            builder.write_bytes(&[0; 8])?;
            if self.profile_type == ColorProfileType::Icc {
                if let Some(icc) = &self.icc_profile {
                    builder.write_u32_le(icc.len().try_into().unwrap())?;
                    builder.write_bytes(icc)?;
                } else {
                    builder.write_u32_le(0)?;
                }
            }
            builder.build()
        }

        fn from_block<M>(mut reader: M) -> MemResult<Self>
        where
            M: MemReader,
        {
            let type_val = reader.read_u16_le()?;
            let flags_val = reader.read_u16_le()?;
            let flags = ColorProfileFlags::from_bits_truncate(flags_val);
            let fixed_gamma = reader.read_u32_le()?;
            let _reserved = reader.read_values::<u8>("reserved", 8)?;

            let profile_type = match type_val {
                0 => ColorProfileType::None,
                1 => ColorProfileType::Srgb,
                2 => ColorProfileType::Icc,
                _ => {
                    return Err(reader
                        .create_invalid_data_error_msg("Invalid color profile type")
                        .into());
                }
            };

            let icc_profile = if profile_type == ColorProfileType::Icc {
                let len = reader.read_u32_le()?;
                Some(reader.read_values::<u8>("icc_profile", len as usize)?)
            } else {
                None
            };

            Ok(ColorProfileChunk {
                profile_type,
                flags,
                fixed_gamma,
                icc_profile,
            })
        }
    }
}

mod external_files {
    use scidev::utils::{
        block::{Block, BlockBuilderFactory},
        mem_reader::{MemReader, Result as MemResult},
    };
    use std::io;

    use crate::formats::aseprite::raw::ExtensionContext;

    use super::ChunkType;

    #[derive(Clone, Copy, Debug)]
    #[repr(u8)]
    pub(super) enum ExternalFileType {
        Palette = 0,
        Tileset = 1,
        ExtensionProperties = 2,
        ExtensionTileManagement = 3,
    }

    #[derive(Clone, Debug)]
    pub(super) struct ExternalFileEntry {
        entry_id: u32,
        file_type: ExternalFileType,
        file_name_or_id: String,
    }

    #[derive(Clone, Debug)]
    pub(super) struct ExternalFilesChunk {
        entries: Vec<ExternalFileEntry>,
    }

    impl ExternalFilesChunk {
        pub(super) fn from_context(ext_context: &ExtensionContext) -> Option<super::ChunkBlock> {
            let entries: Vec<_> = ext_context
                .extension_mappings()
                .map(|(name, id)| ExternalFileEntry {
                    entry_id: id.to_u32(),
                    file_type: ExternalFileType::ExtensionProperties,
                    file_name_or_id: name.clone(),
                })
                .collect();

            if entries.is_empty() {
                return None;
            }

            Some(super::ChunkBlock::from_value(ExternalFilesChunk {
                entries,
            }))
        }
    }

    impl ChunkType for ExternalFilesChunk {
        const CHUNK_TYPE: u16 = 0x2008;

        fn to_block(&self, factory: &BlockBuilderFactory) -> io::Result<Block> {
            let mut builder = factory.create();
            builder.write_u32_le(self.entries.len().try_into().unwrap())?;
            builder.write_bytes(&[0; 8])?;
            for entry in &self.entries {
                builder.write_u32_le(entry.entry_id)?;
                builder.write_u8(entry.file_type as u8)?;
                builder.write_bytes(&[0; 7])?;
                builder.write_u16_le(entry.file_name_or_id.len().try_into().unwrap())?;
                builder.write_bytes(entry.file_name_or_id.as_bytes())?;
            }
            builder.build()
        }

        fn from_block<M>(mut reader: M) -> MemResult<Self>
        where
            M: MemReader,
        {
            let num_entries = reader.read_u32_le()?;
            let _reserved = reader.read_values::<u8>("reserved", 8)?;

            let mut entries = Vec::with_capacity(num_entries as usize);
            for _ in 0..num_entries {
                let entry_id = reader.read_u32_le()?;
                let type_val = reader.read_u8()?;
                let _reserved_entry = reader.read_values::<u8>("reserved_entry", 7)?;
                let name_len = reader.read_u16_le()?;
                let file_name_or_id =
                    String::from_utf8_lossy(&reader.read_values::<u8>("name", name_len as usize)?)
                        .to_string();

                let file_type = match type_val {
                    0 => ExternalFileType::Palette,
                    1 => ExternalFileType::Tileset,
                    2 => ExternalFileType::ExtensionProperties,
                    3 => ExternalFileType::ExtensionTileManagement,
                    _ => {
                        return Err(reader
                            .create_invalid_data_error_msg("Invalid external file type")
                            .into());
                    }
                };

                entries.push(ExternalFileEntry {
                    entry_id,
                    file_type,
                    file_name_or_id,
                });
            }

            Ok(ExternalFilesChunk { entries })
        }
    }
}

impl super::model::Sprite {
    pub fn to_block(&self, builder: &BlockBuilderFactory) -> io::Result<Block> {
        let layers_have_opacity = self
            .contents
            .layers
            .iter()
            .any(|layer| layer.opacity != 255);
        let layers_have_uuids = self
            .contents
            .layers
            .iter()
            .any(|layer| layer.uuid.is_some());

        let (num_indexed_colors, color_depth) = match self.contents.color_depth {
            ColorDepth::Indexed(num_colors) => (num_colors, 8),
            ColorDepth::Grayscale => (0, 16),
            ColorDepth::Rgba => (0, 32),
        };

        let ext_context = ExtensionContext::from_sprite(&self.contents);

        // let header = Header { ... }; // Removed unused initial header

        let mut frames = Vec::new();

        for (frame_idx, frame) in self.contents.frames.iter().enumerate() {
            let is_initial_frame = frame_idx == 0;
            let mut chunks: Vec<ChunkBlock> = Vec::new();

            if is_initial_frame {
                // Write chunks if needed:
                // - ExternalFilesChunk
                if let Some(chunk) = external_files::ExternalFilesChunk::from_context(&ext_context)
                {
                    chunks.push(chunk);
                }

                // - ColorProfileChunk
                if let Some(chunk) =
                    color_profile::ColorProfileChunk::from_backing(&self.contents.color_profile)
                {
                    chunks.push(chunk);
                }

                // Write palette, if changed (always write on frame 0)
                if let Some(chunk) = palette::PaletteChunk::from_backing(&self.contents.palette) {
                    chunks.push(chunk);
                }

                // Write user data for sprite (only on frame 0?) - Spec says UserData for Sprite is usually before Tags
                if let Some(chunk) =
                    user_data::UserDataChunk::from_backing(&ext_context, &self.contents.user_data)
                        .map(ChunkBlock::from_value)
                {
                    chunks.push(chunk);
                }

                chunks.extend(tags::TagsChunk::from_backing(
                    &ext_context,
                    &self.contents.tags,
                ));
                chunks.extend(layer::LayerChunk::from_backing(
                    &ext_context,
                    &self.contents.layers,
                ));
            }

            chunks.extend(cel::CelChunk::create_chunks(
                &ext_context,
                &self.contents.cels,
                frame_idx,
            )?);

            frames.push(build_frame_block(builder, frame.duration_ms, chunks)?);
        }

        let frames_block = builder.concat(frames)?;

        let header = Header {
            file_size: u32::try_from(frames_block.len()).unwrap_or(0) + 128,
            frames_count: u16::try_from(self.contents.frames.len()).unwrap_or(u16::MAX),
            width: self.contents.width,
            height: self.contents.height,
            color_depth,
            flags: HeaderFlags::empty()
                | if layers_have_opacity {
                    HeaderFlags::HAS_LAYER_OPACITY
                } else {
                    HeaderFlags::empty()
                }
                | if layers_have_uuids {
                    HeaderFlags::HAS_LAYER_UUIDS
                } else {
                    HeaderFlags::empty()
                },
            transparent_index: self.contents.transparent_color,
            num_indexed_colors,
            pixel_width: self.contents.pixel_width,
            pixel_height: self.contents.pixel_height,
            grid_x: 0,
            grid_y: 0,
            grid_width: 16,
            grid_height: 16,
            reserved2: [0; _],
        };

        let header_block = header.to_block(builder)?;
        builder.concat([header_block, frames_block])
    }
}

impl super::Property {
    fn read_from<M>(reader: &mut M) -> mem_reader::Result<Self>
    where
        M: MemReader,
    {
        let type_id = reader.read_u16_le()?;
        Self::read_type_from(type_id, reader)
    }

    fn read_type_from<M>(type_id: u16, reader: &mut M) -> mem_reader::Result<Self>
    where
        M: MemReader,
    {
        let result = match type_id {
            1 => Self::Bool(reader.read_u8()? != 0),
            2 => Self::I8(reader.read_i8()?),
            3 => Self::U8(reader.read_u8()?),
            4 => Self::I16(reader.read_i16_le()?),
            5 => Self::U16(reader.read_u16_le()?),
            6 => Self::I32(reader.read_i32_le()?),
            7 => Self::U32(reader.read_u32_le()?),
            8 => Self::I64(reader.read_i64_le()?),
            9 => Self::U64(reader.read_u64_le()?),
            10 => Self::FixedPoint(FixedPoint {
                value: reader.read_i32_le()?,
            }),
            11 => Self::F32(reader.read_f32_le()?),
            12 => Self::F64(reader.read_f64_le()?),
            13 => Self::String(read_string_type(reader)?),
            14 => {
                let x = reader.read_i32_le()?;
                let y = reader.read_i32_le()?;
                Self::Point(Point { x, y })
            }
            15 => {
                let width = reader.read_i32_le()?;
                let height = reader.read_i32_le()?;
                Self::Size(Size { width, height })
            }
            16 => {
                let x = reader.read_i32_le()?;
                let y = reader.read_i32_le()?;
                let width = reader.read_i32_le()?;
                let height = reader.read_i32_le()?;
                Self::Rect(Rect {
                    point: Point { x, y },
                    size: Size { width, height },
                })
            }
            17 => {
                let count = reader.read_u32_le()?;
                let type_id = reader.read_u16_le()?;
                let mut values = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    let elem_type_id = if type_id == 0 {
                        reader.read_u16_le()?
                    } else {
                        type_id
                    };
                    let value = Self::read_type_from(elem_type_id, reader)?;
                    values.push(value);
                }
                Self::Vec(values)
            }
            18 => Self::Map(super::Properties::read_from(reader)?),
            _ => {
                return Err(reader
                    .create_invalid_data_error_msg("Invalid property type")
                    .into());
            }
        };

        Ok(result)
    }

    pub fn write_typed_to(&self, builder: &mut BlockBuilder) -> io::Result<()> {
        builder.write_u16_le(self.type_id())?;
        self.write_untyped_to(builder)
    }

    pub fn write_untyped_to(&self, builder: &mut BlockBuilder) -> io::Result<()> {
        match self {
            Self::Bool(value) => builder.write_u8(u8::from(*value))?,
            Self::I8(value) => builder.write_i8(*value)?,
            Self::U8(value) => builder.write_u8(*value)?,
            Self::I16(value) => builder.write_i16_le(*value)?,
            Self::U16(value) => builder.write_u16_le(*value)?,
            Self::I32(value) => builder.write_i32_le(*value)?,
            Self::U32(value) => builder.write_u32_le(*value)?,
            Self::I64(value) => builder.write_i64_le(*value)?,
            Self::U64(value) => builder.write_u64_le(*value)?,
            Self::F32(value) => builder.write_f32_le(*value)?,
            Self::F64(value) => builder.write_f64_le(*value)?,
            Self::FixedPoint(value) => builder.write_i32_le(value.value)?,
            Self::String(value) => {
                write_string_to(value, builder)?;
                builder
            }
            Self::Point(value) => builder.write_i32_le(value.x)?.write_i32_le(value.y)?,
            Self::Size(value) => builder
                .write_i32_le(value.width)?
                .write_i32_le(value.height)?,
            Self::Rect(value) => builder
                .write_i32_le(value.point.x)?
                .write_i32_le(value.point.y)?
                .write_i32_le(value.size.width)?
                .write_i32_le(value.size.height)?,
            Self::Vec(items) => {
                let count = u32::try_from(items.len())
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Too many items"))?;
                builder.write_u32_le(count)?;
                let group_type_id = if let Some((first, rest)) = items.split_first()
                    && rest.iter().all(|item| item.type_id() == first.type_id())
                {
                    first.type_id()
                } else {
                    0
                };
                builder.write_u16_le(group_type_id)?;
                for item in items {
                    if group_type_id == 0 {
                        item.write_typed_to(builder)?;
                    } else {
                        item.write_untyped_to(builder)?;
                    }
                }
                builder
            }
            Self::Map(properties) => {
                properties.write_to(builder)?;
                builder
            }
            Self::Uuid(value) => builder.write_bytes(value)?,
        };
        Ok(())
    }
}

impl super::Properties {
    pub fn read_from<M>(reader: &mut M) -> mem_reader::Result<Self>
    where
        M: MemReader,
    {
        let count = reader.read_u32_le()?;
        let mut properties = BTreeMap::new();
        for _ in 0..count {
            let key = read_string_type(reader)?;
            let value = super::Property::read_from(reader)?;
            // The existing semantics appear to be that duplicate keys are
            // overwritten.
            properties.insert(key, value);
        }
        Ok(Self { properties })
    }

    pub fn write_to(&self, builder: &mut BlockBuilder) -> io::Result<()> {
        let count = u32::try_from(self.properties.len())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Too many properties"))?;
        builder.write_u32_le(count)?;
        for (key, value) in &self.properties {
            write_string_to(key, builder)?;
            value.write_typed_to(builder)?;
        }
        Ok(())
    }
}
