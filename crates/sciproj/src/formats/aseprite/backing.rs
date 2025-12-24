use std::collections::BTreeMap;

use super::{
    AnimationDirection, BlendMode, CelIndex, Color, ColorDepth, LayerFlags, LayerType, UserData,
};

#[derive(Debug, Clone)]
pub(super) struct TagContents {
    start_frame: u32,
    end_frame: u32,
    name: String,
    animation_direction: AnimationDirection,
    num_repeats: u16,
    user_data: UserData,
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

/// The contents of a cel.
#[derive(Debug, Clone)]
pub(super) struct CelContents {
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) pixel_data: Vec<u8>,
    pub(super) user_data: UserData,
}

#[derive(Debug, Clone)]
pub(super) struct FrameContents {
    pub(super) duration_ms: u16,
    pub(super) palette: Option<Vec<Color>>,
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
    pub(super) external_files: Vec<ExternalFilesEntry>,
}

#[derive(Debug, Clone)]
pub(super) struct PropertyExtensionName {
    name: String,
}

#[derive(Debug, Clone)]
pub(super) enum ExternalFilesEntry {
    Property(PropertyExtensionName),
}
