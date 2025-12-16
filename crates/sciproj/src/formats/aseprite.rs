//! Parsers and builders for Aseprite files.
//!
//! Using the spec at: <https://github.com/aseprite/aseprite/blob/main/docs/ase-file-specs.md>

#![expect(dead_code, clippy::todo)]
use std::io;

use bitflags::bitflags;
use scidev::utils::{block::Block, mem_reader::MemReader};

bitflags! {
    /// Flags for Aseprite frames.
    pub struct HeaderFlags: u16 {
        const HAS_LAYER_OPACITY = 0x0001;
        const HAS_LAYER_GROUP_BLEND = 0x0002;
        const HAS_LAYER_UUIDS = 0x0004;
    }
}

struct Header {
    file_size: u32,
    // magic_number: u16 = 0xA5E0
    frames_count: u16,
    width: u16,
    height: u16,

    /// Color depth in bits per pixel.
    color_depth: u16,

    /// Flags for all layers in the file.
    flags: HeaderFlags,

    // speed: u16 = 0 (deprecated)
    // reserved: [0u32; 2]
    /// The index of the transparent color if mode is indexed.
    ///
    /// Otherwise unused, set to 0
    transparent_index: u8,

    // reserved: [0u8; 3] (fills out the dword alignment)
    /// Number of colors in the palette for indexed color mode.
    num_indexed_colors: u16,

    /// Pixel width (for non-square pixels). With [`pixel_height`], gives the pixel aspect ratio.
    ///
    /// If zero, pixels are square.
    pixel_width: u8,

    /// Pixel width (for non-square pixels). With [`pixel_height`], gives the pixel aspect ratio.
    ///
    /// If zero, pixels are square.
    pixel_height: u8,
    // Following are only used in the case of grids, which we shouldn't need to support.
    // grid_x: i16,
    // grid_y: i16,
    // grid_width: u16,
    // grid_height: u16,

    // padded to 128 bytes
    // reserved: [0u8; 84],
}

struct FrameHeader {
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

struct ChunkHeader {
    /// Size of this chunk in bytes, including this header.
    chunk_size: u32,

    /// The type of this chunk.
    chunk_type: u16,
}

#[derive(Debug, Clone)]
struct ChunkBlock {
    chunk_type: u16,
    data: Block,
}

/// Types that are
trait ChunkValue: Sized {
    const CHUNK_TYPE: u16;

    fn into_block(self) -> Block;
    fn from_block<M>(block: M) -> io::Result<Self>
    where
        M: MemReader;
}

mod layer {
    use bitflags::bitflags;
    use bytes::BufMut;
    use scidev::utils::{block::Block, mem_reader::MemReader};
    use std::io;

    use super::ChunkValue;

    bitflags! {
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        pub struct LayerFlags: u16 {
            const VISIBLE = 0x0001;
            const EDITABLE = 0x0002;
            const LOCK_MOVEMENT = 0x0004;
            const BACKGROUND = 0x0008;
            const PREFER_LINKED_CELS = 0x0010;
            const DISPLAY_COLLAPSED = 0x0020;
            const REFERENCE_LAYER = 0x0040;
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub(super) enum LayerType {
        Normal,
        Group,
        Tilemap { tileset_index: u32 },
    }

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    #[repr(u16)]
    pub(super) enum BlendMode {
        Normal = 0,
        Multiply = 1,
        Screen = 2,
        Overlay = 3,
        Darken = 4,
        Lighten = 5,
        ColorDodge = 6,
        ColorBurn = 7,
        HardLight = 8,
        SoftLight = 9,
        Difference = 10,
        Exclusion = 11,
        Hue = 12,
        Saturation = 13,
        Color = 14,
        Luminosity = 15,
        Addition = 16,
        Subtraction = 17,
        Divide = 18,
    }

    #[derive(Clone, Debug)]
    pub(super) struct LayerChunk {
        flags: LayerFlags,
        layer_type: LayerType,
        child_level: u16,
        // default_width: u16 (ignored)
        // default_height: u16 (ignored)
        blend_mode: BlendMode,
        opacity: u8,
        // padding: [0u8; 3]
        layer_name: String,
        uuid: Option<[u8; 16]>,
    }

    impl ChunkValue for LayerChunk {
        const CHUNK_TYPE: u16 = 0x2004;

        fn into_block(self) -> Block {
            let mut data: Vec<u8> = Vec::new();
            data.put_u16_le(self.flags.bits());
            data.put_u16_le(match &self.layer_type {
                LayerType::Normal => 0,
                LayerType::Group => 1,
                LayerType::Tilemap { .. } => 2,
            });
            data.put_u16_le(self.child_level);
            data.put_u16_le(0); // default_width
            data.put_u16_le(0); // default_height
            data.put_u16_le(self.blend_mode as u16);
            data.put_u8(self.opacity);
            data.put_bytes(0u8, 3);
            data.put_u16_le(self.layer_name.len().try_into().unwrap());
            data.extend_from_slice(self.layer_name.as_bytes());
            if let LayerType::Tilemap { tileset_index } = &self.layer_type {
                data.put_u32_le(*tileset_index);
            }
            if let Some(uuid) = &self.uuid {
                data.extend_from_slice(uuid);
            }
            Block::from_vec(data)
        }

        fn from_block<M>(_reader: M) -> io::Result<Self>
        where
            M: MemReader,
        {
            todo!()
        }
    }
}

mod cel {
    use bytes::BufMut;
    use scidev::utils::{block::Block, mem_reader::MemReader};
    use std::io;

    use super::ChunkValue;

    #[derive(Clone, Debug)]
    pub(super) enum CelType {
        Raw {
            width: u16,
            height: u16,
            pixels: Vec<u8>,
        },
        Linked {
            frame_position: u16,
        },
        Compressed {
            width: u16,
            height: u16,
            data: Vec<u8>,
        },
        CompressedTilemap {
            width: u16,
            height: u16,
            bits_per_tile: u16,
            bitmask_tile_id: u32,
            bitmask_x_flip: u32,
            bitmask_y_flip: u32,
            bitmask_diagonal_flip: u32,
            tiles: Vec<u8>,
        },
    }

    #[derive(Clone, Debug)]
    pub(super) struct CelChunk {
        layer_index: u16,
        x: i16,
        y: i16,
        opacity: u8,
        cel_type: CelType,
        z_index: i16,
    }

    impl ChunkValue for CelChunk {
        const CHUNK_TYPE: u16 = 0x2005;

        fn into_block(self) -> Block {
            let mut data: Vec<u8> = Vec::new();
            data.put_u16_le(self.layer_index);
            data.put_i16_le(self.x);
            data.put_i16_le(self.y);
            data.put_u8(self.opacity);
            let type_val = match &self.cel_type {
                CelType::Raw { .. } => 0,
                CelType::Linked { .. } => 1,
                CelType::Compressed { .. } => 2,
                CelType::CompressedTilemap { .. } => 3,
            };
            data.put_u16_le(type_val);
            data.put_i16_le(self.z_index);
            data.put_bytes(0, 5);

            match self.cel_type {
                CelType::Raw {
                    width,
                    height,
                    pixels,
                } => {
                    data.put_u16_le(width);
                    data.put_u16_le(height);
                    data.extend_from_slice(&pixels);
                }
                CelType::Linked { frame_position } => {
                    data.put_u16_le(frame_position);
                }
                CelType::Compressed {
                    width,
                    height,
                    data: compressed_data,
                } => {
                    data.put_u16_le(width);
                    data.put_u16_le(height);
                    data.extend_from_slice(&compressed_data);
                }
                CelType::CompressedTilemap {
                    width,
                    height,
                    bits_per_tile,
                    bitmask_tile_id,
                    bitmask_x_flip,
                    bitmask_y_flip,
                    bitmask_diagonal_flip,
                    tiles,
                } => {
                    data.put_u16_le(width);
                    data.put_u16_le(height);
                    data.put_u16_le(bits_per_tile);
                    data.put_u32_le(bitmask_tile_id);
                    data.put_u32_le(bitmask_x_flip);
                    data.put_u32_le(bitmask_y_flip);
                    data.put_u32_le(bitmask_diagonal_flip);
                    data.put_bytes(0, 10);
                    data.extend_from_slice(&tiles);
                }
            }
            Block::from_vec(data)
        }

        fn from_block<M>(_reader: M) -> io::Result<Self>
        where
            M: MemReader,
        {
            todo!()
        }
    }
}

mod cel_extra {
    use bytes::BufMut;
    use scidev::utils::{block::Block, mem_reader::MemReader};
    use std::io;

    use super::ChunkValue;

    #[derive(Clone, Debug)]
    pub(super) struct CelExtraChunk {
        flags: u32,     // 1 = precise bounds are set
        precise_x: i32, // FIXED 16.16
        precise_y: i32, // FIXED 16.16
        width: i32,     // FIXED 16.16
        height: i32,    // FIXED 16.16
    }

    impl ChunkValue for CelExtraChunk {
        const CHUNK_TYPE: u16 = 0x2006;

        fn into_block(self) -> Block {
            let mut data: Vec<u8> = Vec::new();
            data.put_u32_le(self.flags);
            data.put_i32_le(self.precise_x);
            data.put_i32_le(self.precise_y);
            data.put_i32_le(self.width);
            data.put_i32_le(self.height);
            data.put_bytes(0, 16);
            Block::from_vec(data)
        }

        fn from_block<M>(_reader: M) -> io::Result<Self>
        where
            M: MemReader,
        {
            todo!()
        }
    }
}

mod tags {
    use bytes::BufMut;
    use scidev::utils::{block::Block, mem_reader::MemReader};
    use std::io;

    use super::ChunkValue;

    #[derive(Clone, Copy, Debug)]
    #[repr(u8)]
    pub(super) enum AnimationDirection {
        Forward = 0,
        Reverse = 1,
        PingPong = 2,
        PingPongReverse = 3,
    }

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

    #[derive(Clone, Debug)]
    pub(super) struct TagsChunk {
        tags: Vec<Tag>,
    }

    impl ChunkValue for TagsChunk {
        const CHUNK_TYPE: u16 = 0x2018;

        fn into_block(self) -> Block {
            let mut data: Vec<u8> = Vec::new();
            data.put_u16_le(self.tags.len().try_into().unwrap());
            data.put_bytes(0, 8);
            for tag in self.tags {
                data.put_u16_le(tag.from_frame);
                data.put_u16_le(tag.to_frame);
                data.put_u8(tag.direction as u8);
                data.put_u16_le(tag.repeat);
                data.put_bytes(0, 6);
                data.put_bytes(0, 3); // deprecated color
                data.put_u8(0); // extra byte
                data.put_u16_le(tag.name.len().try_into().unwrap());
                data.extend_from_slice(tag.name.as_bytes());
            }
            Block::from_vec(data)
        }

        fn from_block<M>(_reader: M) -> io::Result<Self>
        where
            M: MemReader,
        {
            todo!()
        }
    }
}

mod palette {
    use bitflags::bitflags;
    use bytes::BufMut;
    use scidev::utils::{block::Block, mem_reader::MemReader};
    use std::io;

    use super::ChunkValue;

    bitflags! {
        #[derive(Clone, Debug)]
        pub struct PaletteEntryFlags: u16 {
            const HAS_NAME = 0x0001;
        }
    }

    #[derive(Clone, Debug)]
    pub(super) struct PaletteEntry {
        flags: PaletteEntryFlags,
        red: u8,
        green: u8,
        blue: u8,
        alpha: u8,
        name: Option<String>,
    }

    #[derive(Clone, Debug)]
    pub(super) struct PaletteChunk {
        new_palette_size: u32,
        first_color_index: u32,
        last_color_index: u32,
        entries: Vec<PaletteEntry>,
    }

    impl ChunkValue for PaletteChunk {
        const CHUNK_TYPE: u16 = 0x2019;

        fn into_block(self) -> Block {
            let mut data: Vec<u8> = Vec::new();
            data.put_u32_le(self.new_palette_size);
            data.put_u32_le(self.first_color_index);
            data.put_u32_le(self.last_color_index);
            data.put_bytes(0, 8);
            for entry in self.entries {
                data.put_u16_le(entry.flags.bits());
                data.put_u8(entry.red);
                data.put_u8(entry.green);
                data.put_u8(entry.blue);
                data.put_u8(entry.alpha);
                if entry.flags.contains(PaletteEntryFlags::HAS_NAME) {
                    if let Some(name) = &entry.name {
                        data.put_u16_le(name.len().try_into().unwrap());
                        data.extend_from_slice(name.as_bytes());
                    }
                }
            }
            Block::from_vec(data)
        }

        fn from_block<M>(_reader: M) -> io::Result<Self>
        where
            M: MemReader,
        {
            todo!()
        }
    }
}

mod user_data {
    use bitflags::bitflags;
    use bytes::BufMut;
    use scidev::utils::{block::Block, mem_reader::MemReader};
    use std::io;

    use super::ChunkValue;

    bitflags! {
        #[derive(Clone, Debug)]
        pub struct UserDataFlags: u32 {
            const HAS_TEXT = 0x0001;
            const HAS_COLOR = 0x0002;
            const HAS_PROPERTIES = 0x0004;
        }
    }

    #[derive(Clone, Debug)]
    pub(super) struct UserDataChunk {
        flags: UserDataFlags,
        text: Option<String>,
        color: Option<[u8; 4]>, // RGBA
        properties_data: Option<Vec<u8>>,
    }

    impl ChunkValue for UserDataChunk {
        const CHUNK_TYPE: u16 = 0x2020;

        fn into_block(self) -> Block {
            let mut data: Vec<u8> = Vec::new();
            data.put_u32_le(self.flags.bits());
            if let Some(text) = &self.text {
                data.put_u16_le(text.len().try_into().unwrap());
                data.extend_from_slice(text.as_bytes());
            }
            if let Some(color) = self.color {
                data.extend_from_slice(&color);
            }
            if let Some(properties) = &self.properties_data {
                data.extend_from_slice(properties);
            }
            Block::from_vec(data)
        }

        fn from_block<M>(_reader: M) -> io::Result<Self>
        where
            M: MemReader,
        {
            todo!()
        }
    }
}

mod slice {
    use bitflags::bitflags;
    use bytes::BufMut;
    use scidev::utils::{block::Block, mem_reader::MemReader};
    use std::io;

    use super::ChunkValue;

    bitflags! {
        #[derive(Clone, Debug)]
        pub struct SliceFlags: u32 {
            const IS_9_PATCHES = 0x0001;
            const HAS_PIVOT = 0x0002;
        }
    }

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

    #[derive(Clone, Debug)]
    pub(super) struct SliceChunk {
        num_slice_keys: u32,
        flags: SliceFlags,
        name: String,
        keys: Vec<SliceKey>,
    }

    impl ChunkValue for SliceChunk {
        const CHUNK_TYPE: u16 = 0x2022;

        fn into_block(self) -> Block {
            let mut data: Vec<u8> = Vec::new();
            data.put_u32_le(self.keys.len().try_into().unwrap());
            data.put_u32_le(self.flags.bits());
            data.put_u32_le(0); // reserved
            data.put_u16_le(self.name.len().try_into().unwrap());
            data.extend_from_slice(self.name.as_bytes());

            for key in self.keys {
                data.put_u32_le(key.frame_number);
                data.put_i32_le(key.x);
                data.put_i32_le(key.y);
                data.put_u32_le(key.width);
                data.put_u32_le(key.height);
                if let Some((cx, cy, cw, ch)) = key.center {
                    data.put_i32_le(cx);
                    data.put_i32_le(cy);
                    data.put_u32_le(cw);
                    data.put_u32_le(ch);
                }
                if let Some((px, py)) = key.pivot {
                    data.put_i32_le(px);
                    data.put_i32_le(py);
                }
            }
            Block::from_vec(data)
        }

        fn from_block<M>(_reader: M) -> io::Result<Self>
        where
            M: MemReader,
        {
            todo!()
        }
    }
}

mod tileset {
    use bitflags::bitflags;
    use bytes::BufMut;
    use scidev::utils::{block::Block, mem_reader::MemReader};
    use std::io;

    use super::ChunkValue;

    bitflags! {
        #[derive(Clone, Debug)]
        pub struct TilesetFlags: u32 {
            const EXTERNAL_FILE = 0x0001;
            const EMBEDDED = 0x0002;
            const ZERO_IS_EMPTY = 0x0004;
            const MATCH_X_FLIP = 0x0008;
            const MATCH_Y_FLIP = 0x0010;
            const MATCH_D_FLIP = 0x0020;
        }
    }

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

    impl ChunkValue for TilesetChunk {
        const CHUNK_TYPE: u16 = 0x2023;

        fn into_block(self) -> Block {
            let mut data: Vec<u8> = Vec::new();
            data.put_u32_le(self.id);
            data.put_u32_le(self.flags.bits());
            data.put_u32_le(self.num_tiles);
            data.put_u16_le(self.tile_width);
            data.put_u16_le(self.tile_height);
            data.put_i16_le(self.base_index);
            data.put_bytes(0, 14);
            data.put_u16_le(self.name.len().try_into().unwrap());
            data.extend_from_slice(self.name.as_bytes());
            if self.flags.contains(TilesetFlags::EXTERNAL_FILE) {
                data.put_u32_le(self.external_file_id.unwrap_or(0));
                data.put_u32_le(self.external_tileset_id.unwrap_or(0));
            }
            if self.flags.contains(TilesetFlags::EMBEDDED) {
                if let Some(compressed) = &self.compressed_data {
                    data.put_u32_le(compressed.len().try_into().unwrap());
                    data.extend_from_slice(compressed);
                } else {
                    data.put_u32_le(0);
                }
            }
            Block::from_vec(data)
        }

        fn from_block<M>(_reader: M) -> io::Result<Self>
        where
            M: MemReader,
        {
            todo!()
        }
    }
}

mod color_profile {
    use bitflags::bitflags;
    use bytes::BufMut;
    use scidev::utils::{block::Block, mem_reader::MemReader};
    use std::io;

    use super::ChunkValue;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[repr(u16)]
    pub(super) enum ColorProfileType {
        None = 0,
        Srgb = 1,
        Icc = 2,
    }

    bitflags! {
        #[derive(Clone, Debug)]
        pub struct ColorProfileFlags: u16 {
            const FIXED_GAMMA = 0x0001;
        }
    }

    #[derive(Clone, Debug)]
    pub(super) struct ColorProfileChunk {
        profile_type: ColorProfileType,
        flags: ColorProfileFlags,
        fixed_gamma: u32, // FIXED 16.16
        icc_profile: Option<Vec<u8>>,
    }

    impl ChunkValue for ColorProfileChunk {
        const CHUNK_TYPE: u16 = 0x2007;

        fn into_block(self) -> Block {
            let mut data: Vec<u8> = Vec::new();
            data.put_u16_le(self.profile_type as u16);
            data.put_u16_le(self.flags.bits());
            data.put_u32_le(self.fixed_gamma);
            data.put_bytes(0, 8);
            if self.profile_type == ColorProfileType::Icc {
                if let Some(icc) = &self.icc_profile {
                    data.put_u32_le(icc.len().try_into().unwrap());
                    data.extend_from_slice(icc);
                } else {
                    data.put_u32_le(0);
                }
            }
            Block::from_vec(data)
        }

        fn from_block<M>(_reader: M) -> io::Result<Self>
        where
            M: MemReader,
        {
            todo!()
        }
    }
}

mod external_files {
    use bytes::BufMut;
    use scidev::utils::{block::Block, mem_reader::MemReader};
    use std::io;

    use super::ChunkValue;

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

    impl ChunkValue for ExternalFilesChunk {
        const CHUNK_TYPE: u16 = 0x2008;

        fn into_block(self) -> Block {
            let mut data: Vec<u8> = Vec::new();
            data.put_u32_le(self.entries.len().try_into().unwrap());
            data.put_bytes(0, 8);
            for entry in self.entries {
                data.put_u32_le(entry.entry_id);
                data.put_u8(entry.file_type as u8);
                data.put_bytes(0, 7);
                data.put_u16_le(entry.file_name_or_id.len().try_into().unwrap());
                data.extend_from_slice(entry.file_name_or_id.as_bytes());
            }
            Block::from_vec(data)
        }

        fn from_block<M>(_reader: M) -> io::Result<Self>
        where
            M: MemReader,
        {
            todo!()
        }
    }
}
