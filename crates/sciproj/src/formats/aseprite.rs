//! Parsers and builders for Aseprite files.
//!
//! Using the spec at: <https://github.com/aseprite/aseprite/blob/main/docs/ase-file-specs.md>

#![expect(dead_code)]

use std::{collections::BTreeMap, num::NonZeroU32};

use bitflags::bitflags;

// Conceptually, we separate the implementations of the Aseprite processor
// into several layers:
//
// - Logical: The types that represent the logical structure of the data in
//   an ase file. We try to preserve the relationship between pieces of data
//   rather than require a specific structure. These should be plain data
//   structures that do not have any lifetimes.
//
// - Model types: The API provided within the program. This should maximize
//   the readability of the code that operates on the data. This should contain
//   and/or reference the logical types.
//
// - Builder types: The API used to modify and/or create new ase files.
//
// - Structural types: The types that represent the actual file structure of
//   an ase file. This should be as close to the actual file format as possible.
//
// Decoding a file should first parse into the structural types, then convert
// them into the logical types. Encoding should do the opposite.
//
// --- Refined Architecture ---
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
mod raw;
mod tests;

// Export model types for public use
pub use self::model::{CelView, FrameView, LayerView, Sprite};
// Export builder types for public use
pub use self::builder::{CelBuilder, FrameBuilder, LayerBuilder, SpriteBuilder};

#[derive(Debug, Clone, Copy)]
pub enum ColorDepth {
    Rgba,
    Grayscale,
    Indexed(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct Color {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl Color {
    #[must_use]
    pub fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    #[must_use]
    pub fn red(&self) -> u8 {
        self.r
    }

    #[must_use]
    pub fn green(&self) -> u8 {
        self.g
    }

    #[must_use]
    pub fn blue(&self) -> u8 {
        self.b
    }

    #[must_use]
    pub fn alpha(&self) -> u8 {
        self.a
    }
}

#[derive(Debug, Clone)]
pub struct PaletteEntry {
    color: Color,
    name: Option<String>,
}

impl PaletteEntry {
    #[must_use]
    pub fn new(color: Color, name: Option<String>) -> Self {
        Self { color, name }
    }

    #[must_use]
    pub fn color(&self) -> Color {
        self.color
    }

    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct GrayscaleColor {
    gray: u8,
    alpha: u8,
}

impl GrayscaleColor {
    #[must_use]
    pub fn new(gray: u8, alpha: u8) -> Self {
        Self { gray, alpha }
    }

    #[must_use]
    pub fn gray(&self) -> u8 {
        self.gray
    }

    #[must_use]
    pub fn alpha(&self) -> u8 {
        self.alpha
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FixedPoint {
    /// Fixed point value. (16.16)
    value: i32,
}

#[derive(Debug, Clone, Copy)]
pub struct Point {
    x: i32,
    y: i32,
}

#[derive(Debug, Clone, Copy)]
pub struct Point16 {
    x: i16,
    y: i16,
}

impl Point16 {
    #[must_use]
    pub fn new(x: i16, y: i16) -> Self {
        Self { x, y }
    }

    #[must_use]
    pub fn x(&self) -> i16 {
        self.x
    }

    #[must_use]
    pub fn y(&self) -> i16 {
        self.y
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Size {
    width: i32,
    height: i32,
}

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    point: Point,
    size: Size,
}

#[derive(Debug, Clone, Copy)]
pub enum Pixel {
    Rgba(Color),
    Grayscale(GrayscaleColor),
    Indexed(u8),
}

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
pub enum LayerType {
    Normal,
    Group,
    Tilemap { tileset_index: u32 },
}

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

#[derive(Debug, Clone)]
pub enum Property {
    Bool(bool),
    I8(i8),
    U8(u8),
    I16(i16),
    U16(u16),
    I32(i32),
    U32(u32),
    I64(i64),
    U64(u64),
    F32(f32),
    F64(f64),
    FixedPoint(FixedPoint),
    String(String),
    Point(Point),
    Size(Size),
    Rect(Rect),
    Vec(Vec<Property>),
    Map(Properties),
    Uuid([u8; 16]),
}

impl Property {
    fn type_id(&self) -> u16 {
        match self {
            Property::Bool(_) => 1,
            Property::I8(_) => 2,
            Property::U8(_) => 3,
            Property::I16(_) => 4,
            Property::U16(_) => 5,
            Property::I32(_) => 6,
            Property::U32(_) => 7,
            Property::I64(_) => 8,
            Property::U64(_) => 9,
            Property::F32(_) => 10,
            Property::F64(_) => 11,
            Property::FixedPoint(_) => 12,
            Property::String(_) => 13,
            Property::Point(_) => 14,
            Property::Size(_) => 15,
            Property::Rect(_) => 16,
            Property::Vec(_) => 17,
            Property::Map(_) => 18,
            Property::Uuid(_) => 19,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Properties {
    properties: BTreeMap<String, Property>,
}

impl Properties {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum UserDataPropsKey {
    General,
    ForExtension(NonZeroU32),
}

#[derive(Debug, Clone, Copy)]
pub enum AnimationDirection {
    Forward,
    Backward,
    PingPong,
    PingPongReverse,
}

/// The index of a cel within a sprite.
///
/// This consists of the layer and frame indicies. Both must be less than the
/// number of layers and frames in the sprite, respectively.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CelIndex {
    pub layer: u16,
    pub frame: u16,
}
