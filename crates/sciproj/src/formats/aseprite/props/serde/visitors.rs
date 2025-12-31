//! These are literal visitor implementations that can be detected by the
//! internal implementation to specially parse specialized types.
#![expect(unsafe_code, reason = "Needed for castaway visitor specialization.")]

use serde::de::{self, MapAccess, SeqAccess, Visitor};

use crate::formats::aseprite::Point32;

pub(super) struct PointVisitor;

unsafe impl castaway::LifetimeFree for PointVisitor {}

pub(super) trait CastToLiteralVisitor<'de, T>: Visitor<'de> {
    fn visit_literal(self, value: T) -> Self::Value;
}

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

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a point, point-like struct mapping, or a sequence of 2 elements")
    }
}

impl CastToLiteralVisitor<'_, Point32> for PointVisitor {
    fn visit_literal(self, value: Point32) -> Self::Value {
        value
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
