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
