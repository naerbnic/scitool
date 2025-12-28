use std::collections::BTreeMap;

use scidev::utils::block::{Block, CachedMemBlock};

use crate::formats::aseprite::Point16;

use super::{
    AnimationDirection, BlendMode, CelIndex, Color, ColorDepth, LayerFlags, LayerType,
    PaletteEntry, Point, Properties, Size,
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
pub(super) struct CelPixelData {
    pub(super) width: u16,
    pub(super) height: u16,
    pub(super) data: Block,
    pub(super) cached_data: CachedMemBlock,
}

#[derive(Debug, Clone)]
pub(super) enum CelData {
    Pixels(CelPixelData),
    Linked(u16),
    Tilemap, // Reserved for future use
}

/// The contents of a cel.
#[derive(Debug, Clone)]
pub(super) struct CelContents {
    pub(super) position: Point16,
    pub(super) opacity: u8,
    pub(super) contents: CelData,
    pub(super) user_data: UserData,
    pub(super) precise_position: Point,
    pub(super) precise_size: Size,
}

#[derive(Debug, Clone)]
pub(super) struct FrameContents {
    pub(super) duration_ms: u16,
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
pub(super) struct PaletteContents {
    pub(super) entries: Vec<PaletteEntry>,
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
    pub(super) palette: PaletteContents,
    pub(super) user_data: UserData,
}

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Sprite dimensions must be positive, got {width}x{height}")]
    InvalidDimensions { width: u16, height: u16 },
    #[error("Pixel ratio must be positive, got {width}x{height}")]
    InvalidPixelRatio { width: u8, height: u8 },
    #[error(
        "Tag {index} references invalid frame range {from}..={to} (frame count: {frame_count})"
    )]
    InvalidTagRange {
        index: usize,
        from: u32,
        to: u32,
        frame_count: usize,
    },
    #[error("Cel at {index:?} references invalid layer {layer} (layer count: {layer_count})")]
    InvalidCelLayer {
        index: CelIndex,
        layer: usize,
        layer_count: usize,
    },
    #[error("Cel at {index:?} references invalid frame {frame} (frame count: {frame_count})")]
    InvalidCelFrame {
        index: CelIndex,
        frame: usize,
        frame_count: usize,
    },
    #[error("Cel at {index:?} refers to a Group layer")]
    CelOnGroupLayer { index: CelIndex },
    #[error("Cel at {index:?} has {cel_type} content but layer is {layer_type}")]
    CelLayerTypeMismatch {
        index: CelIndex,
        cel_type: &'static str,
        layer_type: &'static str,
    },
    #[error(
        "Linked cel at {index:?} references invalid frame {target} (frame count: {frame_count})"
    )]
    InvalidLinkedCelTarget {
        index: CelIndex,
        target: u16,
        frame_count: usize,
    },
    #[error("Linked cel at {index:?} references itself")]
    LinkedCelSelfReference { index: CelIndex },
}

pub(super) fn validate_sprite(c: &SpriteContents) -> Result<(), ValidationError> {
    // 1. Dimensions
    if c.width == 0 || c.height == 0 {
        return Err(ValidationError::InvalidDimensions {
            width: c.width,
            height: c.height,
        });
    }
    if c.pixel_width == 0 || c.pixel_height == 0 {
        return Err(ValidationError::InvalidPixelRatio {
            width: c.pixel_width,
            height: c.pixel_height,
        });
    }

    // 2. Tags
    for (i, tag) in c.tags.iter().enumerate() {
        if tag.from_frame as usize >= c.frames.len()
            || tag.to_frame as usize >= c.frames.len()
            || tag.from_frame > tag.to_frame
        {
            return Err(ValidationError::InvalidTagRange {
                index: i,
                from: tag.from_frame,
                to: tag.to_frame,
                frame_count: c.frames.len(),
            });
        }
    }

    // 3. Cels
    for (index, cel) in &c.cels {
        if index.layer as usize >= c.layers.len() {
            return Err(ValidationError::InvalidCelLayer {
                index: *index,
                layer: index.layer as usize,
                layer_count: c.layers.len(),
            });
        }
        if index.frame as usize >= c.frames.len() {
            return Err(ValidationError::InvalidCelFrame {
                index: *index,
                frame: index.frame as usize,
                frame_count: c.frames.len(),
            });
        }

        let layer = &c.layers[index.layer as usize];
        match layer.layer_type {
            LayerType::Group => {
                return Err(ValidationError::CelOnGroupLayer { index: *index });
            }
            LayerType::Normal => {
                // Normal layers support Pixels and Linked cels.
                match cel.contents {
                    CelData::Pixels(_) | CelData::Linked(_) => {}
                    CelData::Tilemap => {
                        return Err(ValidationError::CelLayerTypeMismatch {
                            index: *index,
                            cel_type: "Tilemap",
                            layer_type: "Normal",
                        });
                    }
                }
            }
            LayerType::Tilemap { .. } => {
                // Tilemap layers support Tilemap and Linked cels.
                match cel.contents {
                    CelData::Tilemap | CelData::Linked(_) => {}
                    CelData::Pixels(_) => {
                        return Err(ValidationError::CelLayerTypeMismatch {
                            index: *index,
                            cel_type: "Pixels",
                            layer_type: "Tilemap",
                        });
                    }
                }
            }
        }

        if let CelData::Linked(target_frame) = cel.contents {
            if target_frame as usize >= c.frames.len() {
                return Err(ValidationError::InvalidLinkedCelTarget {
                    index: *index,
                    target: target_frame,
                    frame_count: c.frames.len(),
                });
            }
            if target_frame == index.frame {
                return Err(ValidationError::LinkedCelSelfReference { index: *index });
            }
        }
    }

    Ok(())
}
