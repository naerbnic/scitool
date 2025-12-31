//! Parsers and builders for Aseprite files.
//!
//! Using the spec at: <https://github.com/aseprite/aseprite/blob/main/docs/ase-file-specs.md>

#![expect(dead_code)]

use bitflags::bitflags;

// Conceptually, we separate the implementations of the Aseprite processor
// into several layers:
//
// - Structural Layer (`raw_format`):
//   Represents the physical file layout (Headers, Chunks, Bytes).
//   Strictly responsible for Serialization (Write) and Deserialization (Read).
//   These types are transient: parsed from disk, converted to Model, and discarded.
//
// - Logical Backing Layer (`*Contents` types):
//   The authoritative in-memory representation of the data (e.g., `SpriteContents`, `FrameContents`).
//   These structs own the data (Pixels, Strings) but are agnostic to their
//   context (e.g., a `FrameContents` doesn't know its own frame index).
//   This layer should be hidden from the public API to allow internal optimization.
//
// - Model Layer (Public API types):
//   Lightweight, ephemeral handles (e.g., `Frame<'a>`, `Layer<'a>`) that combine:
//   1. A reference to the Backing Layer.
//   2. Contextual metadata (e.g., `frame_index`, `layer_index`).
//   This allows for a rich, fluent API (e.g. `frame.next()`, `frame.duration()`)
//   without cloning heavy data or storing redundant state in the backing store.
//
// - Mutation/Builder Layer:
//   The exclusive API for creating or modifying the Backing Layer.
//   Because the Model has complex cross-cutting invariants (e.g., Tags relying on
//   contiguous Frame indices), direct mutable access to the Model is restricted.
//   Builders/Transactions ensure that operations (like "Insert Frame") automatically
//   adjust dependent state to maintain validity.

mod backing;
mod builder;
mod model;
mod props;
mod raw;
mod tests;

// Export model types for public use
pub use self::model::{
    CelImage, CelPixels, CelView, FrameView, LayerView, PaletteView, PixelSlice, Sprite, TagView,
    TypedPixels,
};
// Export builder types for public use
pub use self::builder::{CelBuilder, FrameBuilder, LayerBuilder, SpriteBuilder};
pub use self::props::{Property, PropertyMap};

/// The color depth (bits per pixel) of the image.
#[derive(Debug, Clone, Copy)]
pub enum ColorDepth {
    /// 32-bit RGBA (Red, Green, Blue, Alpha).
    Rgba,
    /// 16-bit Grayscale (Gray, Alpha).
    Grayscale,
    /// Indexed color (8-bit index into a palette).
    ///
    /// The associated `u16` is the number of colors in the palette (usually 256).
    Indexed(u16),
}

/// An RGBA color value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct Color {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl Color {
    /// Creates a new `Color` from RGBA components.
    #[must_use]
    pub fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Returns the red component.
    #[must_use]
    pub fn red(&self) -> u8 {
        self.r
    }

    /// Returns the green component.
    #[must_use]
    pub fn green(&self) -> u8 {
        self.g
    }

    /// Returns the blue component.
    #[must_use]
    pub fn blue(&self) -> u8 {
        self.b
    }

    /// Returns the alpha component.
    #[must_use]
    pub fn alpha(&self) -> u8 {
        self.a
    }
}

/// An entry in a color palette.
#[derive(Debug, Clone)]
pub struct PaletteEntry {
    color: Color,
    name: Option<String>,
}

impl PaletteEntry {
    /// Creates a new palette entry.
    #[must_use]
    pub fn new(color: Color, name: Option<String>) -> Self {
        Self { color, name }
    }

    /// Returns the color of this entry.
    #[must_use]
    pub fn color(&self) -> Color {
        self.color
    }

    /// Returns the name of this entry, if it has one.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
}

/// A grayscale color value with alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct GrayscaleColor {
    gray: u8,
    alpha: u8,
}

impl GrayscaleColor {
    /// Creates a new `GrayscaleColor`.
    #[must_use]
    pub fn new(gray: u8, alpha: u8) -> Self {
        Self { gray, alpha }
    }

    /// Returns the gray component.
    #[must_use]
    pub fn gray(&self) -> u8 {
        self.gray
    }

    /// Returns the alpha component.
    #[must_use]
    pub fn alpha(&self) -> u8 {
        self.alpha
    }
}

/// A fixed-point number (16.16).
#[derive(Debug, Clone, Copy)]
pub struct FixedI16 {
    /// Fixed point value. (16.16)
    value: i32,
}

/// A 2D point with integer coordinates.
#[derive(Debug, Clone, Copy)]
pub struct Point32 {
    x: i32,
    y: i32,
}

impl Point32 {
    /// Creates a new `Point32`.
    #[must_use]
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Returns the x-coordinate.
    #[must_use]
    pub fn x(&self) -> i32 {
        self.x
    }

    /// Returns the y-coordinate.
    #[must_use]
    pub fn y(&self) -> i32 {
        self.y
    }
}

/// A 2D point with 16-bit integer coordinates.
#[derive(Debug, Clone, Copy)]
pub struct Point16 {
    x: i16,
    y: i16,
}

impl Point16 {
    /// Creates a new `Point16`.
    #[must_use]
    pub fn new(x: i16, y: i16) -> Self {
        Self { x, y }
    }

    /// Returns the x-coordinate.
    #[must_use]
    pub fn x(&self) -> i16 {
        self.x
    }

    /// Returns the y-coordinate.
    #[must_use]
    pub fn y(&self) -> i16 {
        self.y
    }
}

/// A 2D size with integer width and height.
#[derive(Debug, Clone, Copy)]
pub struct Size32 {
    width: i32,
    height: i32,
}

/// A rectangle defined by a top-left point and a size.
#[derive(Debug, Clone, Copy)]
pub struct Rect32 {
    point: Point32,
    size: Size32,
}

#[derive(Debug, Clone)]
pub struct Uuid([u8; 16]);

/// A single pixel value.
#[derive(Debug, Clone, Copy)]
pub enum Pixel {
    /// RGBA pixel.
    Rgba(Color),
    /// Grayscale pixel.
    Grayscale(GrayscaleColor),
    /// Indexed pixel.
    Indexed(u8),
}

bitflags! {
    /// Flags for a layer.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct LayerFlags: u16 {
        /// The layer is visible.
        const VISIBLE = 0x0001;
        /// The layer is editable.
        const EDITABLE = 0x0002;
        /// Movement on this layer is locked.
        const LOCK_MOVEMENT = 0x0004;
        /// This is the background layer.
        const BACKGROUND = 0x0008;
        /// Prefer linked cels when creating new frames (not typically used for reading).
        const PREFER_LINKED_CELS = 0x0010;
        /// The layer group should be displayed collapsed in the UI.
        const DISPLAY_COLLAPSED = 0x0020;
        /// This is a reference layer.
        const REFERENCE_LAYER = 0x0040;
    }
}

/// The type of a layer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LayerType {
    /// A normal image layer.
    Normal,
    /// A group layer that contains other layers.
    Group,
    /// A tilemap layer.
    Tilemap {
        /// The index of the tileset used by this layer.
        tileset_index: u32,
    },
}

/// The blend mode for a layer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u16)]
pub enum BlendMode {
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

/// The direction of an animation tag.
#[derive(Debug, Clone, Copy)]
pub enum AnimationDirection {
    Forward,
    Backward,
    PingPong,
    PingPongReverse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LayerIndex(u16);

impl LayerIndex {
    fn from_u16(value: u16) -> Self {
        Self(value)
    }

    fn as_usize(self) -> usize {
        usize::from(self.0)
    }

    fn as_u16(self) -> u16 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FrameIndex(u16);

impl FrameIndex {
    fn from_u16(value: u16) -> Self {
        Self(value)
    }

    fn as_usize(self) -> usize {
        self.0 as usize
    }

    fn as_u16(self) -> u16 {
        self.0
    }
}

/// The index of a cel within a sprite.
///
/// This consists of the layer and frame indicies. Both must be less than the
/// number of layers and frames in the sprite, respectively.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CelIndex {
    layer: LayerIndex,
    frame: FrameIndex,
}

impl CelIndex {
    #[must_use]
    pub fn new(layer: LayerIndex, frame: FrameIndex) -> Self {
        Self { layer, frame }
    }

    #[must_use]
    pub fn layer(&self) -> LayerIndex {
        self.layer
    }

    #[must_use]
    pub fn frame(&self) -> FrameIndex {
        self.frame
    }
}
