//! These are literal visitor implementations that can be detected by the
//! internal implementation to specially parse specialized types.
#![expect(unsafe_code, reason = "Needed for castaway visitor specialization.")]

use serde::{
    Serialize, Serializer,
    de::{self, MapAccess, SeqAccess, Visitor},
    ser::SerializeStruct,
};

use crate::formats::aseprite::{FixedI16, Point32, Rect32, Size32, Uuid};

pub(super) struct FixedI16Visitor;

impl<'de> Visitor<'de> for FixedI16Visitor {
    type Value = FixedI16;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("an f32, or f64")
    }

    fn visit_f32<E>(self, value: f32) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        #[allow(clippy::cast_possible_truncation)]
        Ok(FixedI16 {
            value: (f64::from(value) * 65536.0).round() as i32,
        })
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        #[allow(clippy::cast_possible_truncation)]
        Ok(FixedI16 {
            value: (value * 65536.0).round() as i32,
        })
    }

    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_f64(self)
    }
}

pub(super) struct PointVisitor;

impl<'de> Visitor<'de> for PointVisitor {
    type Value = Point32;

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let x = seq
            .next_element()?
            .ok_or_else(|| de::Error::invalid_length(0, &"expected 2 elements"))?;
        let y = seq
            .next_element()?
            .ok_or_else(|| de::Error::invalid_length(1, &"expected 2 elements"))?;
        Ok(Point32::new(x, y))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut x = None;
        let mut y = None;
        while let Some((key, value)) = map.next_entry()? {
            match key {
                "x" => {
                    if x.is_some() {
                        return Err(de::Error::duplicate_field("x"));
                    }
                    x = Some(value);
                }
                "y" => {
                    if y.is_some() {
                        return Err(de::Error::duplicate_field("y"));
                    }
                    y = Some(value);
                }
                _ => {
                    return Err(de::Error::unknown_field(key, &[]));
                }
            }
        }
        let x = x.ok_or_else(|| de::Error::missing_field("x"))?;
        let y = y.ok_or_else(|| de::Error::missing_field("y"))?;
        Ok(Point32::new(x, y))
    }

    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_seq(self)
    }

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a point, point-like struct mapping, or a sequence of 2 elements")
    }
}

pub(super) struct SizeVisitor;

impl<'de> Visitor<'de> for SizeVisitor {
    type Value = Size32;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a size, size-like struct mapping, or a sequence of 2 elements")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let width = seq
            .next_element()?
            .ok_or_else(|| de::Error::invalid_length(0, &"expected 2 elements"))?;
        let height = seq
            .next_element()?
            .ok_or_else(|| de::Error::invalid_length(1, &"expected 2 elements"))?;
        Ok(Size32::new(width, height))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut width = None;
        let mut height = None;
        while let Some((key, value)) = map.next_entry()? {
            match key {
                "width" => {
                    if width.is_some() {
                        return Err(de::Error::duplicate_field("width"));
                    }
                    width = Some(value);
                }
                "height" => {
                    if height.is_some() {
                        return Err(de::Error::duplicate_field("height"));
                    }
                    height = Some(value);
                }
                _ => {
                    return Err(de::Error::unknown_field(key, &[]));
                }
            }
        }
        let width = width.ok_or_else(|| de::Error::missing_field("width"))?;
        let height = height.ok_or_else(|| de::Error::missing_field("height"))?;
        Ok(Size32::new(width, height))
    }

    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_seq(self)
    }
}

pub(super) struct RectVisitor;

impl<'de> Visitor<'de> for RectVisitor {
    type Value = Rect32;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a rect, rect-like struct mapping, or a sequence of 4 elements")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let origin = seq
            .next_element()?
            .ok_or_else(|| de::Error::invalid_length(0, &"expected 4 elements"))?;
        let size = seq
            .next_element()?
            .ok_or_else(|| de::Error::invalid_length(2, &"expected 4 elements"))?;
        Ok(Rect32::new(origin, size))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut origin = None;
        let mut size = None;
        while let Some(key) = map.next_key()? {
            match key {
                "origin" => {
                    if origin.is_some() {
                        return Err(de::Error::duplicate_field("origin"));
                    }
                    origin = Some(map.next_value()?);
                }
                "size" => {
                    if size.is_some() {
                        return Err(de::Error::duplicate_field("size"));
                    }
                    size = Some(map.next_value()?);
                }
                _ => {
                    return Err(de::Error::unknown_field(key, &[]));
                }
            }
        }
        let origin = origin.ok_or_else(|| de::Error::missing_field("origin"))?;
        let size = size.ok_or_else(|| de::Error::missing_field("size"))?;
        Ok(Rect32::new(origin, size))
    }

    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_map(self)
    }
}

pub(super) struct UuidVisitor;

impl<'de> Visitor<'de> for UuidVisitor {
    type Value = Uuid;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a UUID")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut uuid_bytes = [0u8; 16];
        for (i, uuid_byte) in uuid_bytes.iter_mut().enumerate() {
            *uuid_byte = seq
                .next_element()?
                .ok_or_else(|| de::Error::invalid_length(i, &"expected 16 elements"))?;
        }
        if seq.next_element::<u8>()?.is_some() {
            return Err(de::Error::invalid_length(16, &"expected 16 elements"));
        }
        Ok(Uuid(uuid_bytes))
    }

    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_seq(self)
    }
}

impl<'de> de::Deserialize<'de> for FixedI16 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_newtype_struct("FixedI16", FixedI16Visitor)
    }
}

impl<'de> de::Deserialize<'de> for Point32 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_newtype_struct("Point32", PointVisitor)
    }
}

impl<'de> de::Deserialize<'de> for Size32 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_newtype_struct("Size32", SizeVisitor)
    }
}

impl<'de> de::Deserialize<'de> for Rect32 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_newtype_struct("Rect32", RectVisitor)
    }
}

impl<'de> de::Deserialize<'de> for Uuid {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_newtype_struct("Uuid", UuidVisitor)
    }
}

unsafe impl castaway::LifetimeFree for FixedI16Visitor {}
unsafe impl castaway::LifetimeFree for PointVisitor {}
unsafe impl castaway::LifetimeFree for SizeVisitor {}
unsafe impl castaway::LifetimeFree for RectVisitor {}
unsafe impl castaway::LifetimeFree for UuidVisitor {}

impl Serialize for FixedI16 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_newtype_struct("FixedI16", &FixedI16Surrogate(*self))
    }
}

impl Serialize for Point32 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_newtype_struct("Point32", &PointSurrogate(*self))
    }
}

impl Serialize for Size32 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_newtype_struct("Size32", &SizeSurrogate(*self))
    }
}

impl Serialize for Rect32 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_newtype_struct("Rect32", &RectSurrogate(*self))
    }
}

impl Serialize for Uuid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_newtype_struct("Uuid", &UuidSurrogate(self.clone()))
    }
}

pub(super) struct FixedI16Surrogate(pub FixedI16);

impl Serialize for FixedI16Surrogate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_f64(self.0.as_f64())
    }
}

pub(super) struct PointSurrogate(pub Point32);

impl Serialize for PointSurrogate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_seq([self.0.x, self.0.y])
    }
}

pub(super) struct SizeSurrogate(pub Size32);

impl Serialize for SizeSurrogate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_seq([self.0.width, self.0.height])
    }
}

pub(super) struct RectSurrogate(pub Rect32);

impl Serialize for RectSurrogate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut struct_ser = serializer.serialize_struct("Rect32", 2)?;
        struct_ser.serialize_field("origin", &self.0.origin)?;
        struct_ser.serialize_field("size", &self.0.size)?;
        struct_ser.end()
    }
}

pub(super) struct UuidSurrogate(pub Uuid);

impl Serialize for UuidSurrogate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(&self.0.0)
    }
}

unsafe impl castaway::LifetimeFree for FixedI16Surrogate {}
unsafe impl castaway::LifetimeFree for PointSurrogate {}
unsafe impl castaway::LifetimeFree for SizeSurrogate {}
unsafe impl castaway::LifetimeFree for RectSurrogate {}
unsafe impl castaway::LifetimeFree for UuidSurrogate {}
