use std::marker::PhantomData;
use std::ops::Range;
use std::{io, mem};

use crate::formats::aseprite::{
    BlendMode, CelIndex, Color, ColorDepth, GrayscaleColor, Point, Properties,
    backing::{
        CelContents, CelData, CelPixelData, SpriteContents, ValidationError, validate_sprite,
    },
};

use scidev::utils::block::MemBlock;

pub struct Sprite {
    pub(super) contents: SpriteContents,
}

impl Sprite {
    pub(super) fn new(contents: SpriteContents) -> Result<Self, ValidationError> {
        validate_sprite(&contents)?;
        Ok(Self { contents })
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
            CelData::Pixels(pixels) => CelImage::RawPixels(CelPixels {
                inner: pixels,
                color_depth: self.sprite.contents.color_depth,
            }),
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
    RawPixels(CelPixels<'a>),
    Tilemap,
}

#[derive(Clone, Copy)]
pub struct CelPixels<'a> {
    inner: &'a CelPixelData,
    color_depth: ColorDepth,
}

impl CelPixels<'_> {
    #[must_use]
    pub fn width(&self) -> u16 {
        self.inner.width
    }

    #[must_use]
    pub fn height(&self) -> u16 {
        self.inner.height
    }

    #[must_use]
    pub fn color_mode(&self) -> ColorDepth {
        self.color_depth
    }

    fn raw_bytes(&self) -> io::Result<MemBlock> {
        self.inner
            .cached_data
            .get_or_else(|| self.inner.data.open_mem(..))
    }

    pub fn as_rgba(&self) -> io::Result<PixelSlice<Color>> {
        if matches!(self.color_depth, ColorDepth::Rgba) {
            PixelSlice::new(self.raw_bytes()?)
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Cel is not RGBA",
            ))
        }
    }

    pub fn as_indexed(&self) -> io::Result<PixelSlice<u8>> {
        if matches!(self.color_depth, ColorDepth::Indexed(_)) {
            PixelSlice::new(self.raw_bytes()?)
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Cel is not Indexed",
            ))
        }
    }

    /// Returns a typed view of the pixel data based on the color depth.
    pub fn as_pixels(&self) -> io::Result<TypedPixels> {
        // We open the block as a MemBlock (loading it into memory if needed).
        // Since we are creating a view, we need to ensure we have the data.
        // raw_bytes() returns a MemBlock which is RefCounted, so cloning it is cheap.
        let block = self.raw_bytes()?;

        match self.color_depth {
            ColorDepth::Rgba => Ok(TypedPixels::Rgba(PixelSlice::new(block)?)),
            ColorDepth::Grayscale => Ok(TypedPixels::Grayscale(PixelSlice::new(block)?)),
            ColorDepth::Indexed(_) => Ok(TypedPixels::Indexed(PixelSlice::new(block)?)),
        }
    }
}

/// An enumeration of typed pixel views corresponding to the image's color mode.
#[derive(Debug)]
pub enum TypedPixels {
    Rgba(PixelSlice<Color>),
    Grayscale(PixelSlice<GrayscaleColor>),
    Indexed(PixelSlice<u8>),
}

/// A typed view of pixel data.
///
/// This type behaves like a slice `&[T]` via [`Deref`], providing safe, strongly-typed
/// access to the underlying pixels.
#[derive(Debug)]
pub struct PixelSlice<T> {
    block: MemBlock,
    _marker: PhantomData<T>,
}

impl<T> PixelSlice<T> {
    /// Creates a new `PixelSlice` from a `MemBlock`.
    ///
    /// Returns an error if:
    /// - The type `T` has an alignment greater than 1.
    /// - The block's size is not a multiple of `size_of::<T>()`.
    fn new(block: MemBlock) -> io::Result<Self> {
        if mem::align_of::<T>() != 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "PixelSlice only supports types with alignment 1, but {} has alignment {}",
                    std::any::type_name::<T>(),
                    mem::align_of::<T>()
                ),
            ));
        }

        if !block.len().is_multiple_of(mem::size_of::<T>()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Block size {} is not a multiple of pixel size {}",
                    block.len(),
                    mem::size_of::<T>()
                ),
            ));
        }

        Ok(Self {
            block,
            _marker: PhantomData,
        })
    }
}

impl<T> std::ops::Deref for PixelSlice<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        let len = self.block.len() / mem::size_of::<T>();
        // SAFETY:
        // 1. We checked in `new` that `T` has alignment 1, so any `u8` pointer is correctly aligned for `T`.
        // 2. We checked in `new` that the block length is a multiple of `size_of::<T>()`.
        // 3. `MemBlock` (usually) provides a valid pointer and length.
        // 4. The lifetime of the slice is tied to `&self`, which owns the `MemBlock`, ensuring the data remains valid.
        #[allow(unsafe_code)]
        unsafe {
            std::slice::from_raw_parts(self.block.as_ptr().cast::<T>(), len)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pixel_slice_rgba() {
        let colors = [
            Color::from_rgba(255, 0, 0, 255),
            Color::from_rgba(0, 255, 0, 255),
            Color::from_rgba(0, 0, 255, 255),
        ];

        // Ensure Color size is 4
        assert_eq!(mem::size_of::<Color>(), 4);
        assert_eq!(mem::align_of::<Color>(), 1);

        let mut bytes = Vec::new();
        for c in colors {
            bytes.push(c.red());
            bytes.push(c.green());
            bytes.push(c.blue());
            bytes.push(c.alpha());
        }

        let block = MemBlock::from_vec(bytes);
        let slice = PixelSlice::<Color>::new(block).expect("Should create PixelSlice");

        assert_eq!(slice.len(), 3);
        assert_eq!(slice[0], colors[0]);
        assert_eq!(slice[1], colors[1]);
        assert_eq!(slice[2], colors[2]);
    }

    #[test]
    fn test_pixel_slice_grayscale() {
        // GrayscaleColor { gray: u8, alpha: u8 }
        // size: 2, align: 1
        assert_eq!(mem::size_of::<GrayscaleColor>(), 2);
        assert_eq!(mem::align_of::<GrayscaleColor>(), 1);

        let data: Vec<u8> = vec![
            128, 255, // Gray, Alpha
            0, 255, // Black, Alpha
            255, 0, // White, Transparent
        ];

        let block = MemBlock::from_vec(data);
        let slice = PixelSlice::<GrayscaleColor>::new(block).expect("Should create PixelSlice");

        assert_eq!(slice.len(), 3);
        assert_eq!(slice[0], GrayscaleColor::new(128, 255));
        assert_eq!(slice[1], GrayscaleColor::new(0, 255));
        assert_eq!(slice[2], GrayscaleColor::new(255, 0));
    }

    #[test]
    fn test_pixel_slice_indexed() {
        let data: Vec<u8> = vec![0, 1, 2, 3, 4, 255];
        let block = MemBlock::from_vec(data.clone());
        let slice = PixelSlice::<u8>::new(block).expect("Should create PixelSlice");

        assert_eq!(slice.len(), 6);
        assert_eq!(slice[0], 0);
        assert_eq!(slice[5], 255);
    }

    #[test]
    fn test_pixel_slice_size_mismatch() {
        // Color is 4 bytes. Provide 5 bytes.
        let data: Vec<u8> = vec![0, 0, 0, 0, 1];
        let block = MemBlock::from_vec(data);
        let result = PixelSlice::<Color>::new(block);
        assert!(result.is_err());
    }

    #[test]
    fn test_pixel_slice_alignment_check() {
        // We need a type with alignment > 1.
        // u16 has alignment 2.
        let data: Vec<u8> = vec![0, 0, 0, 0];
        let block = MemBlock::from_vec(data);
        let result = PixelSlice::<u16>::new(block);
        // Expect error because align_of::<u16>() == 2 != 1
        assert!(result.is_err());
    }
}
