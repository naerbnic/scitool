use std::collections::BTreeMap;

use scidev::utils::block::{Block, CachedMemBlock};

use crate::formats::aseprite::{FrameIndex, LayerIndex, Point16};

use super::{
    AnimationDirection, BlendMode, CelIndex, Color, ColorDepth, LayerFlags, LayerType,
    PaletteEntry, Point32, Size32, props::PropertyMap,
};

/// Keys for user data properties.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum UserDataPropsKey {
    /// General properties not associated with a specific extension.
    General,
    /// Properties associated with a specific extension.
    Extension(String),
}

/// User data associated with various Aseprite elements.
#[derive(Debug, Clone, Default)]
pub(super) struct UserDataContents {
    pub(super) text: Option<String>,
    pub(super) color: Option<Color>,
    pub(super) properties: BTreeMap<UserDataPropsKey, PropertyMap>,
}

impl UserDataContents {
    pub(super) fn extension_names(&self) -> impl Iterator<Item = &str> + '_ {
        self.properties.keys().filter_map(|k| match k {
            UserDataPropsKey::Extension(s) => Some(s.as_str()),
            UserDataPropsKey::General => None,
        })
    }
}

/// The backing data for an animation tag.
#[derive(Debug, Clone)]
pub(super) struct TagContents {
    pub(super) from_frame: u32,
    pub(super) to_frame: u32,
    pub(super) name: String,
    pub(super) color: Color,
    pub(super) direction: AnimationDirection,
    pub(super) user_data: UserDataContents,
}

/// The backing data for a layer.
#[derive(Debug, Clone)]
pub(super) struct LayerContents {
    pub(super) name: String,
    pub(super) flags: LayerFlags,
    pub(super) layer_type: LayerType,
    pub(super) blend_mode: BlendMode,
    pub(super) opacity: u8,
    pub(super) uuid: Option<[u8; 16]>,
    pub(super) user_data: UserDataContents,
}

/// Raw pixel data for a cel, including potential on-disk caching.
#[derive(Debug, Clone)]
pub(super) struct CelPixelData {
    pub(super) width: u16,
    pub(super) height: u16,
    pub(super) data: Block,
    pub(super) cached_data: CachedMemBlock,
}

/// The type of data contained in a cel.
#[derive(Debug, Clone)]
pub(super) enum CelData {
    /// Raw pixel data.
    Pixels(CelPixelData),
    /// A link to another frame's cel on the same layer.
    Linked(FrameIndex),
    /// Tilemap data (reserved for future use).
    Tilemap, // Reserved for future use
}

/// The contents of a cel.
#[derive(Debug, Clone)]
pub(super) struct CelContents {
    pub(super) position: Point16,
    pub(super) opacity: u8,
    pub(super) contents: CelData,
    pub(super) user_data: UserDataContents,
    pub(super) precise_position: Point32,
    pub(super) precise_size: Size32,
}

/// The backing data for a frame.
#[derive(Debug, Clone)]
pub(super) struct FrameContents {
    pub(super) duration_ms: u16,
}

/// An ICC color profile.
#[derive(Debug, Clone)]
pub(super) struct IccProfile {
    pub(super) data: Vec<u8>,
}

/// The color profile of the sprite.
#[derive(Debug, Clone)]
pub(super) enum ColorProfile {
    /// No color profile.
    None,
    /// sRGB color profile.
    Srgb,
    /// Embedded ICC profile.
    Icc(IccProfile),
}

/// The backing data for the palette.
#[derive(Debug, Clone)]
pub(super) struct PaletteContents {
    pub(super) entries: Vec<PaletteEntry>,
}

/// The authoritative in-memory representation of an Aseprite sprite.
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
    pub(super) user_data: UserDataContents,
}

/// Errors that can occur during sprite validation.
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    /// The sprite has invalid dimensions.
    #[error("Sprite dimensions must be positive, got {width}x{height}")]
    InvalidDimensions { width: u16, height: u16 },
    /// The sprite has an invalid pixel ratio.
    #[error("Pixel ratio must be positive, got {width}x{height}")]
    InvalidPixelRatio { width: u8, height: u8 },
    /// A tag references an invalid frame range.
    #[error(
        "Tag {index} references invalid frame range {from}..={to} (frame count: {frame_count})"
    )]
    InvalidTagRange {
        index: usize,
        from: u32,
        to: u32,
        frame_count: usize,
    },
    /// A cel references an invalid layer.
    #[error("Cel at {index:?} references invalid layer {layer:?} (layer count: {layer_count})")]
    InvalidCelLayer {
        index: CelIndex,
        layer: LayerIndex,
        layer_count: usize,
    },
    /// A cel references an invalid frame.
    #[error("Cel at {index:?} references invalid frame {frame:?} (frame count: {frame_count})")]
    InvalidCelFrame {
        index: CelIndex,
        frame: FrameIndex,
        frame_count: usize,
    },
    /// A cel is assigned to a group layer.
    #[error("Cel at {index:?} refers to a Group layer")]
    CelOnGroupLayer { index: CelIndex },
    /// A cel has content type incompatible with its layer type.
    #[error("Cel at {index:?} has {cel_type} content but layer is {layer_type}")]
    CelLayerTypeMismatch {
        index: CelIndex,
        cel_type: &'static str,
        layer_type: &'static str,
    },
    /// A linked cel references an invalid target frame.
    #[error(
        "Linked cel at {index:?} references invalid frame {target:?} (frame count: {frame_count})"
    )]
    InvalidLinkedCelTarget {
        index: CelIndex,
        target: FrameIndex,
        frame_count: usize,
    },
    /// A linked cel references itself.
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
        if index.layer.as_usize() >= c.layers.len() {
            return Err(ValidationError::InvalidCelLayer {
                index: *index,
                layer: index.layer,
                layer_count: c.layers.len(),
            });
        }
        if index.frame.as_usize() >= c.frames.len() {
            return Err(ValidationError::InvalidCelFrame {
                index: *index,
                frame: index.frame,
                frame_count: c.frames.len(),
            });
        }

        let layer = &c.layers[index.layer.as_usize()];
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
            if target_frame.as_usize() >= c.frames.len() {
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

impl SpriteContents {
    pub(super) fn visit_user_data<'a, F>(&'a self, mut f: F)
    where
        F: FnMut(&'a UserDataContents),
    {
        f(&self.user_data);
        for layer in &self.layers {
            f(&layer.user_data);
        }
        for tag in &self.tags {
            f(&tag.user_data);
        }
        for cel in self.cels.values() {
            f(&cel.user_data);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visit_user_data() {
        let mut sprite = SpriteContents {
            color_depth: ColorDepth::Rgba,
            width: 10,
            height: 10,
            pixel_width: 1,
            pixel_height: 1,
            transparent_color: 0,
            frames: vec![],
            layers: vec![],
            tags: vec![],
            cels: BTreeMap::new(),
            color_profile: ColorProfile::None,
            palette: PaletteContents { entries: vec![] },
            user_data: UserDataContents {
                text: Some("Sprite".to_string()),
                color: None,
                properties: BTreeMap::new(),
            },
        };

        // Add a layer
        sprite.layers.push(LayerContents {
            name: "L1".to_string(),
            flags: LayerFlags::empty(),
            layer_type: LayerType::Normal,
            blend_mode: BlendMode::Normal,
            opacity: 255,
            uuid: None,
            user_data: UserDataContents {
                text: Some("Layer".to_string()),
                color: None,
                properties: BTreeMap::new(),
            },
        });

        // Add a tag
        sprite.tags.push(TagContents {
            from_frame: 0,
            to_frame: 1,
            name: "T1".to_string(),
            color: Color::from_rgba(0, 0, 0, 0),
            direction: AnimationDirection::Forward,
            user_data: UserDataContents {
                text: Some("Tag".to_string()),
                color: None,
                properties: BTreeMap::new(),
            },
        });

        // Add a cel
        let cel_idx = CelIndex {
            layer: LayerIndex::from_u16(0),
            frame: FrameIndex::from_u16(0),
        };
        sprite.cels.insert(
            cel_idx,
            CelContents {
                position: Point16 { x: 0, y: 0 },
                opacity: 255,
                contents: CelData::Tilemap, // Dummy
                precise_position: Point32 { x: 0, y: 0 },
                precise_size: Size32 {
                    width: 0,
                    height: 0,
                },
                user_data: UserDataContents {
                    text: Some("Cel".to_string()),
                    color: None,
                    properties: BTreeMap::new(),
                },
            },
        );

        let mut visited = Vec::new();
        sprite.visit_user_data(|ud| {
            if let Some(txt) = &ud.text {
                visited.push(txt.clone());
            }
        });

        assert!(visited.contains(&"Sprite".to_string()));
        assert!(visited.contains(&"Layer".to_string()));
        assert!(visited.contains(&"Tag".to_string()));
        assert!(visited.contains(&"Cel".to_string()));
        assert_eq!(visited.len(), 4);
    }
}
