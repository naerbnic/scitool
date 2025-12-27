use std::ops::Range;

use crate::formats::aseprite::{
    BlendMode, CelIndex, Color, ColorDepth, Point, Properties,
    backing::{CelContents, CelData, CelPixels, SpriteContents},
};

pub struct Sprite {
    pub(super) contents: SpriteContents,
}

impl Sprite {
    pub(super) fn new(contents: SpriteContents) -> Self {
        // TODO: Verification logic
        Self { contents }
    }

    #[must_use]
    pub fn width(&self) -> u16 {
        self.contents.width
    }

    #[must_use]
    pub fn height(&self) -> u16 {
        self.contents.height
    }

    #[must_use]
    pub fn pixel_width(&self) -> u8 {
        self.contents.pixel_width
    }

    #[must_use]
    pub fn pixel_height(&self) -> u8 {
        self.contents.pixel_height
    }

    #[must_use]
    pub fn color_depth(&self) -> ColorDepth {
        self.contents.color_depth
    }

    pub fn frames(&self) -> impl Iterator<Item = FrameView<'_>> {
        self.contents
            .frames
            .iter()
            .enumerate()
            .map(|(index, _)| FrameView {
                sprite: self,
                index: u16::try_from(index).unwrap(),
            })
    }

    #[must_use]
    pub fn frame(&self, index: usize) -> Option<FrameView<'_>> {
        if index < self.contents.frames.len() {
            Some(FrameView {
                sprite: self,
                index: u16::try_from(index).unwrap(),
            })
        } else {
            None
        }
    }

    pub fn layers(&self) -> impl Iterator<Item = LayerView<'_>> {
        self.contents
            .layers
            .iter()
            .enumerate()
            .map(|(index, _)| LayerView {
                sprite: self,
                index: u16::try_from(index).unwrap(),
            })
    }

    #[must_use]
    pub fn layer(&self, index: usize) -> Option<LayerView<'_>> {
        if index < self.contents.layers.len() {
            Some(LayerView {
                sprite: self,
                index: u16::try_from(index).unwrap(),
            })
        } else {
            None
        }
    }

    pub fn tags(&self) -> impl Iterator<Item = TagView<'_>> {
        self.contents
            .tags
            .iter()
            .enumerate()
            .map(|(index, _)| TagView {
                sprite: self,
                index,
            })
    }

    pub fn cels(&self) -> impl Iterator<Item = CelView<'_>> {
        self.contents.cels.iter().map(|(&index, contents)| CelView {
            sprite: self,
            index,
            contents,
        })
    }

    #[must_use]
    pub fn cel(&self, layer: u16, frame: u16) -> Option<CelView<'_>> {
        let index = CelIndex { layer, frame };
        if self.contents.cels.contains_key(&index) {
            Some(CelView {
                sprite: self,
                index,
                contents: &self.contents.cels[&index],
            })
        } else {
            None
        }
    }

    #[must_use]
    pub fn palette(&self) -> Option<PaletteView<'_>> {
        match self.contents.color_depth {
            ColorDepth::Indexed(_) => Some(PaletteView { sprite: self }),
            _ => None,
        }
    }

    #[must_use]
    pub fn color(&self) -> Option<Color> {
        self.contents.user_data.color
    }

    #[must_use]
    pub fn data(&self) -> Option<&str> {
        self.contents.user_data.text.as_deref()
    }

    pub fn properties(&self) -> impl Iterator<Item = (&str, &Properties)> {
        self.contents
            .user_data
            .properties
            .iter()
            .map(|(k, v)| (k.as_str(), v))
    }
}

#[derive(Clone, Copy)]
pub struct FrameView<'a> {
    sprite: &'a Sprite,
    index: u16,
}

impl<'a> FrameView<'a> {
    #[must_use]
    pub fn index(&self) -> u16 {
        self.index
    }

    #[must_use]
    pub fn duration(&self) -> u16 {
        self.sprite.contents.frames[self.index as usize].duration_ms
    }

    pub fn cels(&self) -> impl Iterator<Item = CelView<'a>> {
        self.sprite
            .cels()
            .filter(|cel| cel.index().frame == self.index)
    }

    #[must_use]
    pub fn cel(&self, layer_index: u16) -> Option<CelView<'a>> {
        self.sprite.cel(layer_index, self.index)
    }

    #[must_use]
    pub fn sprite(&self) -> &'a Sprite {
        self.sprite
    }
}

#[derive(Clone, Copy)]
pub struct LayerView<'a> {
    sprite: &'a Sprite,
    index: u16,
}

impl<'a> LayerView<'a> {
    #[must_use]
    pub fn index(&self) -> u16 {
        self.index
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.sprite.contents.layers[self.index as usize].name
    }

    #[must_use]
    pub fn is_visible(&self) -> bool {
        self.sprite.contents.layers[self.index as usize]
            .flags
            .contains(crate::formats::aseprite::LayerFlags::VISIBLE)
    }

    #[must_use]
    pub fn opacity(&self) -> u8 {
        self.sprite.contents.layers[self.index as usize].opacity
    }

    #[must_use]
    pub fn blend_mode(&self) -> BlendMode {
        self.sprite.contents.layers[self.index as usize].blend_mode
    }

    pub fn cels(&self) -> impl Iterator<Item = CelView<'a>> {
        self.sprite
            .cels()
            .filter(|cel| cel.index().layer == self.index)
    }

    #[must_use]
    pub fn cel(&self, frame_index: u16) -> Option<CelView<'a>> {
        self.sprite.cel(self.index, frame_index)
    }

    #[must_use]
    pub fn sprite(&self) -> &'a Sprite {
        self.sprite
    }

    #[must_use]
    pub fn color(&self) -> Option<Color> {
        self.sprite.contents.layers[self.index as usize]
            .user_data
            .color
    }

    #[must_use]
    pub fn data(&self) -> Option<&str> {
        self.sprite.contents.layers[self.index as usize]
            .user_data
            .text
            .as_deref()
    }

    pub fn properties(&self) -> impl Iterator<Item = (&str, &Properties)> {
        self.sprite.contents.layers[self.index as usize]
            .user_data
            .properties
            .iter()
            .map(|(k, v)| (k.as_str(), v))
    }
}

#[derive(Clone, Copy)]
pub struct TagView<'a> {
    sprite: &'a Sprite,
    index: usize,
}

impl<'a> TagView<'a> {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.sprite.contents.tags[self.index].name
    }

    #[must_use]
    pub fn range(&self) -> Range<usize> {
        let tag = &self.sprite.contents.tags[self.index];
        (tag.from_frame as usize)..(tag.to_frame as usize + 1)
    }

    #[must_use]
    pub fn direction(&self) -> crate::formats::aseprite::AnimationDirection {
        self.sprite.contents.tags[self.index].direction
    }

    pub fn frames(&self) -> impl Iterator<Item = FrameView<'a>> {
        let range = self.range();
        range.map(|i| FrameView {
            sprite: self.sprite,
            index: u16::try_from(i).unwrap(),
        })
    }

    #[must_use]
    pub fn color(&self) -> Option<Color> {
        self.sprite.contents.tags[self.index].user_data.color
    }

    #[must_use]
    pub fn data(&self) -> Option<&str> {
        self.sprite.contents.tags[self.index]
            .user_data
            .text
            .as_deref()
    }

    pub fn properties(&self) -> impl Iterator<Item = (&str, &Properties)> {
        self.sprite.contents.tags[self.index]
            .user_data
            .properties
            .iter()
            .map(|(k, v)| (k.as_str(), v))
    }
}

#[derive(Clone, Copy)]
pub struct CelView<'a> {
    sprite: &'a Sprite,
    index: CelIndex,
    contents: &'a CelContents,
}

impl<'a> CelView<'a> {
    #[must_use]
    pub fn index(&self) -> CelIndex {
        self.index
    }

    #[must_use]
    pub fn position(&self) -> Point {
        self.contents.position
    }

    #[must_use]
    pub fn opacity(&self) -> u8 {
        self.contents.opacity
    }

    #[must_use]
    pub fn linked_cel(&self) -> Option<CelView<'a>> {
        if let CelData::Linked(frame_index) = &self.contents.contents {
            self.sprite.cel(self.index.layer, *frame_index)
        } else {
            None
        }
    }

    #[must_use]
    pub fn image(&self) -> CelImage<'a> {
        self.resolve_image(self.contents)
    }

    fn resolve_image(&self, contents: &'a CelContents) -> CelImage<'a> {
        match &contents.contents {
            CelData::Pixels(pixels) => CelImage::RawPixels(RawPixels { inner: pixels }),
            CelData::Linked(frame_idx) => {
                // Safety: Aseprite validity rules ensure no cycles and valid references.
                self.sprite
                    .cel(self.index.layer, *frame_idx)
                    .expect("Links are validated")
                    .image()
            }
            CelData::Tilemap => CelImage::Tilemap,
        }
    }

    #[must_use]
    pub fn layer(&self) -> LayerView<'a> {
        LayerView {
            sprite: self.sprite,
            index: self.index.layer,
        }
    }

    #[must_use]
    pub fn frame(&self) -> FrameView<'a> {
        FrameView {
            sprite: self.sprite,
            index: self.index.frame,
        }
    }

    #[must_use]
    pub fn sprite(&self) -> &'a Sprite {
        self.sprite
    }

    #[must_use]
    pub fn color(&self) -> Option<Color> {
        self.contents.user_data.color
    }

    #[must_use]
    pub fn data(&self) -> Option<&str> {
        self.contents.user_data.text.as_deref()
    }

    pub fn properties(&self) -> impl Iterator<Item = (&str, &Properties)> {
        self.contents
            .user_data
            .properties
            .iter()
            .map(|(k, v)| (k.as_str(), v))
    }
}

#[derive(Clone, Copy)]
pub struct PaletteView<'a> {
    sprite: &'a Sprite,
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "Keep by reference in case we need to add data later"
)]
impl PaletteView<'_> {
    #[must_use]
    pub fn len(&self) -> usize {
        self.sprite.contents.palette.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sprite.contents.palette.entries.is_empty()
    }

    #[must_use]
    pub fn color(&self, index: usize) -> Option<Color> {
        self.sprite
            .contents
            .palette
            .entries
            .get(index)
            .map(|e| e.color)
    }
}

pub enum CelImage<'a> {
    RawPixels(RawPixels<'a>),
    Tilemap,
}

#[derive(Clone, Copy)]
pub struct RawPixels<'a> {
    inner: &'a CelPixels,
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "Keep by reference in case we need to add more data to the base type later."
)]
impl RawPixels<'_> {
    #[must_use]
    pub fn width(&self) -> u16 {
        self.inner.width
    }

    #[must_use]
    pub fn height(&self) -> u16 {
        self.inner.height
    }

    // Stub for pixel data access
    // pub fn data(&self) -> ...
}
