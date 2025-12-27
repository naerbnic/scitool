use std::collections::BTreeMap;

use scidev::utils::block::Block;

use super::{
    AnimationDirection, BlendMode, CelIndex, Color, ColorDepth, LayerFlags, LayerType, Point,
    Properties, Size,
};

#[derive(Debug, Clone, Default)]
pub(super) struct UserData {
    pub(super) text: Option<String>,
    pub(super) color: Option<Color>,
    pub(super) properties: BTreeMap<String, Properties>,
}

#[derive(Debug, Clone)]
pub(super) struct TagContents {
    pub(super) from_frame: u32,
    pub(super) to_frame: u32,
    pub(super) name: String,
    pub(super) color: Color,
    pub(super) direction: AnimationDirection,
    pub(super) user_data: UserData,
}

#[derive(Debug, Clone)]
pub(super) struct LayerContents {
    pub(super) name: String,
    pub(super) flags: LayerFlags,
    pub(super) layer_type: LayerType,
    pub(super) blend_mode: BlendMode,
    pub(super) opacity: u8,
    pub(super) uuid: Option<[u8; 16]>,
    pub(super) user_data: UserData,
}

#[derive(Debug, Clone)]
pub(super) struct CelPixels {
    pub(super) width: u16,
    pub(super) height: u16,
    pub(super) data: Block,
}

#[derive(Debug, Clone)]
pub(super) enum CelData {
    Pixels(CelPixels),
    Linked(u16),
    Tilemap, // Reserved for future use
}

/// The contents of a cel.
#[derive(Debug, Clone)]
pub(super) struct CelContents {
    pub(super) position: Point,
    pub(super) opacity: u8,
    pub(super) contents: CelData,
    pub(super) user_data: UserData,
    pub(super) precise_position: Point,
    pub(super) precise_size: Size,
}

#[derive(Debug, Clone)]
pub(super) struct FrameContents {
    pub(super) duration_ms: u16,
    pub(super) user_data: UserData,
}

#[derive(Debug, Clone)]
pub(super) struct IccProfile {
    pub(super) data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub(super) enum ColorProfile {
    None,
    Srgb,
    Icc(IccProfile),
}

#[derive(Debug, Clone)]
pub(super) struct SpriteContents {
    pub(super) color_depth: ColorDepth,
    pub(super) width: u16,
    pub(super) height: u16,
    pub(super) pixel_height: u8,
    pub(super) pixel_width: u8,
    pub(super) transparent_color: u8,
    pub(super) frames: Vec<FrameContents>,
    pub(super) layers: Vec<LayerContents>,
    pub(super) tags: Vec<TagContents>,
    pub(super) cels: BTreeMap<CelIndex, CelContents>,
    pub(super) color_profile: ColorProfile,
    pub(super) user_data: UserData,
}
