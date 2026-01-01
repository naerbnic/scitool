use std::collections::{BTreeMap, btree_map};

use scidev::utils::block::{Block, CachedMemBlock};

use crate::formats::aseprite::{
    AnimationDirection, BlendMode, CelIndex, Color, ColorDepth, FrameIndex, LayerFlags, LayerIndex,
    LayerType, PaletteEntry, Point16, Point32, Size32,
    backing::{
        CelContents, CelData, CelPixelData, ColorProfile, FrameContents, LayerContents,
        PaletteContents, SpriteContents, TagContents, UserDataContents, ValidationError,
    },
    model::Sprite,
    props::Property,
};

pub struct FrameBuilder<'a> {
    index: FrameIndex,
    contents: &'a mut FrameContents,
}

impl FrameBuilder<'_> {
    #[must_use]
    pub fn index(&self) -> FrameIndex {
        self.index
    }

    pub fn set_duration(&mut self, duration_ms: u16) {
        self.contents.duration_ms = duration_ms;
    }
}

pub struct LayerBuilder<'a> {
    index: LayerIndex,
    contents: &'a mut LayerContents,
}

impl LayerBuilder<'_> {
    #[must_use]
    pub fn index(&self) -> LayerIndex {
        self.index
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        self.contents.name = name.into();
    }

    pub fn set_flags(&mut self, flags: LayerFlags) {
        self.contents.flags = flags;
    }

    pub fn set_type(&mut self, layer_type: LayerType) {
        self.contents.layer_type = layer_type;
    }

    /// Set blend mode.
    pub fn set_blend_mode(&mut self, mode: BlendMode) {
        self.contents.blend_mode = mode;
    }

    pub fn set_opacity(&mut self, opacity: u8) {
        self.contents.opacity = opacity;
    }
}

pub struct CelBuilder<'a> {
    index: CelIndex,
    contents: &'a mut CelContents,
}

impl CelBuilder<'_> {
    #[must_use]
    pub fn index(&self) -> CelIndex {
        self.index
    }

    pub fn set_position(&mut self, x: i16, y: i16) {
        self.contents.position = Point16 { x, y };
    }

    pub fn set_opacity(&mut self, opacity: u8) {
        self.contents.opacity = opacity;
    }

    /// Sets the cel content to pixel data.
    pub fn set_image(&mut self, width: u16, height: u16, data: impl Into<Block>) {
        self.contents.contents = CelData::Pixels(CelPixelData {
            width,
            height,
            data: data.into(),
            cached_data: CachedMemBlock::new(),
        });
    }

    /// Sets the cel content to be a link to another frame.
    pub fn set_linked(&mut self, frame_index: FrameIndex) {
        self.contents.contents = CelData::Linked(frame_index);
    }

    pub fn set_user_data(&mut self, text: Option<String>, color: Option<Color>) {
        self.contents.user_data.text = text;
        self.contents.user_data.color = color;
    }

    pub fn set_extension_property(&mut self, extension_id: &str, key: &str, value: Property) {
        let entry = self
            .contents
            .user_data
            .properties
            .entry(
                crate::formats::aseprite::backing::UserDataPropsKey::Extension(
                    extension_id.to_string(),
                ),
            )
            .or_default();
        entry.set(key.to_string(), value);
    }

    pub fn set_general_property(&mut self, key: &str, value: Property) {
        let entry = self
            .contents
            .user_data
            .properties
            .entry(crate::formats::aseprite::backing::UserDataPropsKey::General)
            .or_default();
        entry.set(key.to_string(), value);
    }
}

pub struct SpriteBuilder {
    contents: SpriteContents,
}

impl SpriteBuilder {
    #[must_use]
    pub fn new(color_mode: ColorDepth) -> Self {
        Self {
            contents: SpriteContents {
                color_depth: color_mode,
                width: 0,
                height: 0,
                pixel_height: 1,
                pixel_width: 1,
                transparent_color: 0,
                frames: Vec::new(),
                layers: Vec::new(),
                tags: Vec::new(),
                cels: BTreeMap::new(),
                color_profile: ColorProfile::None,
                palette: PaletteContents {
                    entries: Vec::new(),
                },
                user_data: UserDataContents::default(),
            },
        }
    }

    pub fn add_frame(&mut self) -> FrameBuilder<'_> {
        let frame_index = self.contents.frames.len();
        let frame = FrameContents { duration_ms: 0 };
        self.contents.frames.push(frame);
        FrameBuilder {
            index: FrameIndex::from_u16(u16::try_from(frame_index).unwrap()),
            contents: &mut self.contents.frames[frame_index],
        }
    }

    pub fn add_layer(&mut self) -> LayerBuilder<'_> {
        let layer_index = self.contents.layers.len();
        let layer = LayerContents {
            name: String::new(),
            flags: LayerFlags::empty(),
            layer_type: LayerType::Normal,
            blend_mode: BlendMode::Normal,
            opacity: 255,
            uuid: None,
            user_data: UserDataContents::default(),
        };
        self.contents.layers.push(layer);
        LayerBuilder {
            index: LayerIndex::from_u16(u16::try_from(layer_index).unwrap()),
            contents: &mut self.contents.layers[layer_index],
        }
    }

    pub fn add_cel(&mut self, layer: LayerIndex, frame: FrameIndex) -> CelBuilder<'_> {
        let index = CelIndex { layer, frame };
        let cel_ref = match self.contents.cels.entry(index) {
            btree_map::Entry::Vacant(vac) => vac.insert(CelContents {
                position: Point16 { x: 0, y: 0 },
                opacity: 255,
                contents: CelData::Pixels(CelPixelData {
                    width: 0,
                    height: 0,
                    data: Block::from_vec(Vec::new()),
                    cached_data: CachedMemBlock::new(),
                }),
                user_data: UserDataContents::default(),
                precise_position: Point32 { x: 0, y: 0 },
                precise_size: Size32 {
                    width: 0,
                    height: 0,
                },
            }),
            btree_map::Entry::Occupied(occ) => occ.into_mut(),
        };
        CelBuilder {
            index,
            contents: cel_ref,
        }
    }

    pub fn add_tag(
        &mut self,
        from_frame: u32,
        to_frame: u32,
        name: String,
        direction: AnimationDirection,
    ) {
        self.contents.tags.push(TagContents {
            from_frame,
            to_frame,
            name,
            color: Color::from_rgba(0, 0, 0, 255), // Default black
            direction,
            user_data: UserDataContents::default(),
        });
    }

    pub fn set_palette(&mut self, entries: Vec<PaletteEntry>) {
        self.contents.palette.entries = entries;
    }

    pub fn set_transparent_color(&mut self, color: u8) {
        self.contents.transparent_color = color;
    }

    /// Validates and builds the [`Sprite`].
    pub fn build(self) -> Result<Sprite, ValidationError> {
        Sprite::new(self.contents)
    }

    pub fn set_width(&mut self, width: u16) {
        self.contents.width = width;
    }

    pub fn set_extension_property(&mut self, extension_id: &str, key: &str, value: Property) {
        let entry = self
            .contents
            .user_data
            .properties
            .entry(
                crate::formats::aseprite::backing::UserDataPropsKey::Extension(
                    extension_id.to_string(),
                ),
            )
            .or_default();
        entry.set(key.to_string(), value);
    }
    pub fn set_height(&mut self, height: u16) {
        self.contents.height = height;
    }

    pub fn set_pixel_ratio(&mut self, pixel_width: u8, pixel_height: u8) {
        self.contents.pixel_width = pixel_width;
        self.contents.pixel_height = pixel_height;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_basic() {
        let mut builder = SpriteBuilder::new(ColorDepth::Rgba);
        builder.set_width(32);
        builder.set_height(32);

        let mut frame = builder.add_frame();
        frame.set_duration(100);

        let mut layer = builder.add_layer();
        layer.set_name("Layer 1");
        layer.set_opacity(128);

        let mut cel = builder.add_cel(LayerIndex::from_u16(0), FrameIndex::from_u16(0));
        cel.set_opacity(200);
        cel.set_image(32, 32, Block::from_vec(vec![0u8; 32 * 32 * 4]));

        let result = builder.build();
        assert!(result.is_ok());
        let sprite = result.unwrap();
        assert_eq!(sprite.width(), 32);
        assert_eq!(sprite.height(), 32);
        assert_eq!(sprite.frame(0).unwrap().duration(), 100);
        assert_eq!(sprite.layer(0).unwrap().name(), "Layer 1");
        // Verify other properties if accessors exist
    }

    #[test]
    fn test_builder_invalid_dimensions() {
        let mut builder = SpriteBuilder::new(ColorDepth::Rgba);
        builder.set_width(0); // Invalid
        builder.set_height(32);
        builder.add_frame();

        match builder.build() {
            Err(ValidationError::InvalidDimensions { width, .. }) => assert_eq!(width, 0),
            _ => panic!("Expected InvalidDimensions error"),
        }
    }

    #[test]
    fn test_builder_invalid_tag_range() {
        let mut builder = SpriteBuilder::new(ColorDepth::Rgba);
        builder.set_width(10);
        builder.set_height(10);
        builder.add_frame(); // Index 0

        // Add tag referencing frame 1 (invalid, only 0 exists)
        builder.add_tag(0, 1, "Tag1".to_string(), AnimationDirection::Forward);

        match builder.build() {
            Err(ValidationError::InvalidTagRange { index, to, .. }) => {
                assert_eq!(index, 0);
                assert_eq!(to, 1);
            }
            _ => panic!("Expected InvalidTagRange error"),
        }
    }

    #[test]
    fn test_builder_layer_consistency() {
        let mut builder = SpriteBuilder::new(ColorDepth::Rgba);
        builder.set_width(10);
        builder.set_height(10);
        builder.add_frame();

        // Add a Group layer (index 0)
        let mut layer_builder = builder.add_layer();
        layer_builder.set_type(LayerType::Group);

        // Add a pixel cel to Group layer (invalid)
        // add_cel(layer, frame)
        builder.add_cel(LayerIndex::from_u16(0), FrameIndex::from_u16(0));

        match builder.build() {
            Err(ValidationError::CelOnGroupLayer { index }) => {
                assert_eq!(index.layer, LayerIndex::from_u16(0));
                assert_eq!(index.frame, FrameIndex::from_u16(0));
            }
            _ => panic!("Expected CelOnGroupLayer error"),
        }
    }
}
