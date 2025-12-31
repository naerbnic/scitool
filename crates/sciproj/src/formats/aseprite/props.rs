use std::collections::BTreeMap;

use crate::formats::aseprite::{FixedI16, Point32, Rect32, Size32};

mod serde;

/// A user data property value.
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
    FixedPoint(FixedI16),
    String(String),
    Point(Point32),
    Size(Size32),
    Rect(Rect32),
    Vec(Vec<Property>),
    Map(PropertyMap),
    Uuid([u8; 16]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum PropertyTag {
    Bool,
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    F32,
    F64,
    FixedPoint,
    String,
    Point,
    Size,
    Rect,
    Vec,
    Map,
    Uuid,
}

#[derive(Debug, thiserror::Error)]
#[error("Invalid property tag: {0}")]
pub(super) struct InvalidPropertyTag(u16);

impl PropertyTag {
    pub(super) fn from_u16(id: u16) -> Result<Self, InvalidPropertyTag> {
        match id {
            1 => Ok(PropertyTag::Bool),
            2 => Ok(PropertyTag::I8),
            3 => Ok(PropertyTag::U8),
            4 => Ok(PropertyTag::I16),
            5 => Ok(PropertyTag::U16),
            6 => Ok(PropertyTag::I32),
            7 => Ok(PropertyTag::U32),
            8 => Ok(PropertyTag::I64),
            9 => Ok(PropertyTag::U64),
            10 => Ok(PropertyTag::FixedPoint),
            11 => Ok(PropertyTag::F32),
            12 => Ok(PropertyTag::F64),
            13 => Ok(PropertyTag::String),
            14 => Ok(PropertyTag::Point),
            15 => Ok(PropertyTag::Size),
            16 => Ok(PropertyTag::Rect),
            17 => Ok(PropertyTag::Vec),
            18 => Ok(PropertyTag::Map),
            19 => Ok(PropertyTag::Uuid),
            _ => Err(InvalidPropertyTag(id)),
        }
    }

    pub(super) fn to_u16(self) -> u16 {
        match self {
            PropertyTag::Bool => 1,
            PropertyTag::I8 => 2,
            PropertyTag::U8 => 3,
            PropertyTag::I16 => 4,
            PropertyTag::U16 => 5,
            PropertyTag::I32 => 6,
            PropertyTag::U32 => 7,
            PropertyTag::I64 => 8,
            PropertyTag::U64 => 9,
            PropertyTag::FixedPoint => 10,
            PropertyTag::F32 => 11,
            PropertyTag::F64 => 12,
            PropertyTag::String => 13,
            PropertyTag::Point => 14,
            PropertyTag::Size => 15,
            PropertyTag::Rect => 16,
            PropertyTag::Vec => 17,
            PropertyTag::Map => 18,
            PropertyTag::Uuid => 19,
        }
    }
}

impl Property {
    pub(super) fn type_id(&self) -> PropertyTag {
        match self {
            Property::Bool(_) => PropertyTag::Bool,
            Property::I8(_) => PropertyTag::I8,
            Property::U8(_) => PropertyTag::U8,
            Property::I16(_) => PropertyTag::I16,
            Property::U16(_) => PropertyTag::U16,
            Property::I32(_) => PropertyTag::I32,
            Property::U32(_) => PropertyTag::U32,
            Property::I64(_) => PropertyTag::I64,
            Property::U64(_) => PropertyTag::U64,
            Property::F32(_) => PropertyTag::F32,
            Property::F64(_) => PropertyTag::F64,
            Property::FixedPoint(_) => PropertyTag::FixedPoint,
            Property::String(_) => PropertyTag::String,
            Property::Point(_) => PropertyTag::Point,
            Property::Size(_) => PropertyTag::Size,
            Property::Rect(_) => PropertyTag::Rect,
            Property::Vec(_) => PropertyTag::Vec,
            Property::Map(_) => PropertyTag::Map,
            Property::Uuid(_) => PropertyTag::Uuid,
        }
    }
}

/// A collection of user-defined properties.
#[derive(Debug, Clone)]
pub struct PropertyMap {
    properties: BTreeMap<String, Property>,
}

impl PropertyMap {
    #[must_use]
    pub fn new() -> Self {
        Self {
            properties: BTreeMap::new(),
        }
    }

    pub fn set(&mut self, key: String, value: Property) {
        self.properties.insert(key, value);
    }

    #[must_use]
    pub fn properties(&self) -> &BTreeMap<String, Property> {
        &self.properties
    }
}

impl Default for PropertyMap {
    fn default() -> Self {
        Self::new()
    }
}
