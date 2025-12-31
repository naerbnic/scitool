//! An implementation of serde Serialize and Deserialize for `PropertyMap`

use std::borrow::Cow;
use std::cell::Cell;
use std::io::{self, Seek, SeekFrom, Write};

use bytes::{Buf as _, TryGetError};
use itertools::Itertools;
use scidev::utils::data_writer::DataWriterExt as _;
use serde::de::{self, DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::forward_to_deserialize_any;
use serde::ser::{self, SerializeMap as _, Serializer};

use crate::formats::aseprite::Point32;
use crate::formats::aseprite::props::PropertyTag;

mod visitors;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Invalid property type: {tag:?}")]
    InvalidPropertyType { tag: u16 },

    #[error("not enough bytes for tag. Expected: {expected}, actual remaining: {actual}")]
    NotEnoughBytes { expected: usize, actual: usize },

    #[error("Invalid UTF-8")]
    InvalidUTF8,

    #[error("Unsupported visitor for type: {type_name}")]
    UnsupportedType { type_name: Cow<'static, str> },

    #[error("Too many elements in sequence")]
    TooManyElements,

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

impl Error {
    #[expect(
        clippy::needless_pass_by_value,
        reason = "Ergonomics for Result::map_err()"
    )]
    fn from_try_get(err: TryGetError) -> Self {
        Error::NotEnoughBytes {
            expected: err.requested,
            actual: err.available,
        }
    }

    fn other<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        Error::Other(msg.to_string())
    }

    fn unsupported_type(name: impl Into<Cow<'static, str>>) -> Self {
        Error::UnsupportedType {
            type_name: name.into(),
        }
    }
}

impl serde::ser::Error for Error {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        Error::Other(msg.to_string())
    }
}

impl serde::de::Error for Error {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        Error::Other(msg.to_string())
    }
}

struct SeqReader<'de, 'reader> {
    tag: Option<PropertyTag>,
    remaining_elements: u32,
    reader: &'reader mut &'de [u8],
}

impl<'de> SeqAccess<'de> for &mut SeqReader<'de, '_> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        if self.remaining_elements == 0 {
            return Ok(None);
        }

        self.remaining_elements -= 1;

        // If no tag is set, each element has its own tag in the prefix.
        let tag = if let Some(tag) = self.tag {
            tag
        } else {
            let raw_tag = self.reader.try_get_u16_le().map_err(Error::from_try_get)?;
            PropertyTag::from_u16(raw_tag)
                .map_err(|_| Error::InvalidPropertyType { tag: raw_tag })?
        };

        seed.deserialize(PropertyDeserializer::new_tagged(tag, self.reader))
            .map(Some)
    }
}

struct MapReader<'de, 'reader> {
    remaining_elements: u32,
    reader: &'reader mut &'de [u8],
}

impl<'de> MapAccess<'de> for MapReader<'de, '_> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: DeserializeSeed<'de>,
    {
        if self.remaining_elements == 0 {
            return Ok(None);
        }

        self.remaining_elements -= 1;

        // A key is represented as a literal untagged string.
        seed.deserialize(PropertyDeserializer::new_tagged(
            PropertyTag::String,
            self.reader,
        ))
        .map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        // Values are represented as a tagged value.
        seed.deserialize(PropertyDeserializer::new(self.reader))
    }
}

struct EnumReader<'de, 'reader> {
    reader: &'reader mut &'de [u8],
}

impl<'de> de::EnumAccess<'de> for EnumReader<'de, '_> {
    type Error = Error;

    type Variant = Self;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        seed.deserialize(PropertyDeserializer::new_tagged(
            PropertyTag::String,
            self.reader,
        ))
        .map(|variant| (variant, self))
    }
}

impl<'de> de::VariantAccess<'de> for EnumReader<'de, '_> {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Self::Error> {
        todo!()
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        seed.deserialize(PropertyDeserializer::new(self.reader))
    }

    fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        PropertyDeserializer::new(self.reader).deserialize_tuple(len, visitor)
    }

    fn struct_variant<V>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        PropertyDeserializer::new(self.reader).deserialize_struct("", fields, visitor)
    }
}

struct PropertyDeserializer<'de, 'reader> {
    tag: Option<PropertyTag>,
    reader: &'reader mut &'de [u8],
}

impl<'de, 'reader> PropertyDeserializer<'de, 'reader> {
    pub(crate) fn new(reader: &'reader mut &'de [u8]) -> Self {
        Self { tag: None, reader }
    }

    pub(crate) fn new_tagged(tag: PropertyTag, reader: &'reader mut &'de [u8]) -> Self {
        Self {
            tag: Some(tag),
            reader,
        }
    }

    pub(crate) fn read_string(&mut self) -> Result<String, Error> {
        let len = usize::try_from(self.reader.try_get_u32_le().map_err(Error::from_try_get)?)
            .map_err(Error::other)?;

        let mut allocated = vec![0u8; len];
        self.reader
            .try_copy_to_slice(&mut allocated)
            .map_err(Error::from_try_get)?;

        let parsed_str =
            std::string::String::from_utf8(allocated).map_err(|_| Error::InvalidUTF8)?;
        Ok(parsed_str)
    }

    /// Reads the next tag in the stream, if it isn't already set, or hasn't already been read.
    pub(crate) fn read_tag(&mut self) -> Result<PropertyTag, Error> {
        let tag = if let Some(tag) = self.tag {
            tag
        } else {
            let raw_tag = self.reader.try_get_u16_le().map_err(Error::from_try_get)?;
            let tag = PropertyTag::from_u16(raw_tag)
                .map_err(|_| Error::InvalidPropertyType { tag: raw_tag })?;
            self.tag = Some(tag);
            tag
        };

        Ok(tag)
    }
}

impl<'de> Deserializer<'de> for PropertyDeserializer<'de, '_> {
    type Error = Error;

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct identifier ignored_any
    }

    fn deserialize_any<V>(mut self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let tag = self.read_tag()?;

        Ok(match tag {
            PropertyTag::Bool => visitor.visit_bool::<Error>(self.reader.get_u8() != 0)?,
            PropertyTag::I8 => visitor.visit_i8::<Error>(self.reader.get_i8())?,
            PropertyTag::U8 => visitor.visit_u8::<Error>(self.reader.get_u8())?,
            PropertyTag::I16 => visitor.visit_i16::<Error>(self.reader.get_i16_le())?,
            PropertyTag::U16 => visitor.visit_u16::<Error>(self.reader.get_u16_le())?,
            PropertyTag::I32 => visitor.visit_i32::<Error>(self.reader.get_i32_le())?,
            PropertyTag::U32 => visitor.visit_u32::<Error>(self.reader.get_u32_le())?,
            PropertyTag::I64 => visitor.visit_i64::<Error>(self.reader.get_i64_le())?,
            PropertyTag::U64 => visitor.visit_u64::<Error>(self.reader.get_u64_le())?,
            PropertyTag::FixedPoint => {
                return Err(Error::unsupported_type("FixedPoint"));
            }
            PropertyTag::F32 => visitor.visit_f32::<Error>(self.reader.get_f32_le())?,
            PropertyTag::F64 => visitor.visit_f64::<Error>(self.reader.get_f64_le())?,
            PropertyTag::String => visitor.visit_string::<Error>(self.read_string()?)?,
            PropertyTag::Point => {
                let x = self.reader.get_i32_le();
                let y = self.reader.get_i32_le();
                if castaway::cast!(visitor, self::visitors::PointVisitor).is_ok() {
                    // SAFETY:
                    //
                    // - Castaway ensures that the visitor here is of type `PointVisitor`, which has
                    //   a value type of `Point32`.
                    // - `Point32` is a POD type that is already copyable, so copying it is safe.
                    #[expect(unsafe_code)]
                    unsafe {
                        std::mem::transmute_copy::<_, V::Value>(&Point32 { x, y })
                    }
                } else {
                    return Err(Error::unsupported_type("Point32"));
                }
            }
            PropertyTag::Size => {
                return Err(Error::unsupported_type("Size32"));
            }
            PropertyTag::Rect => {
                return Err(Error::unsupported_type("Rect32"));
            }
            PropertyTag::Vec => {
                let count = self.reader.try_get_u32_le().map_err(Error::from_try_get)?;
                let raw_tag = self.reader.try_get_u16_le().map_err(Error::from_try_get)?;
                let tag = if raw_tag == 0 {
                    None
                } else {
                    Some(
                        PropertyTag::from_u16(raw_tag)
                            .map_err(|_| Error::InvalidPropertyType { tag: raw_tag })?,
                    )
                };
                let mut reader = SeqReader {
                    tag,
                    remaining_elements: count,
                    reader: self.reader,
                };
                let result = visitor.visit_seq(&mut reader)?;

                // Check that we've consumed all sequence elements.
                if reader.remaining_elements != 0 {
                    return Err(de::Error::invalid_length(
                        usize::try_from(count).unwrap(),
                        &format!("{} elements", count - reader.remaining_elements).as_str(),
                    ));
                }
                result
            }
            PropertyTag::Map => {
                let count = self.reader.try_get_u32_le().map_err(Error::from_try_get)?;
                visitor.visit_map(MapReader {
                    remaining_elements: count,
                    reader: self.reader,
                })?
            }
            PropertyTag::Uuid => {
                return Err(Error::unsupported_type("UUID"));
            }
        })
    }

    fn deserialize_enum<V>(
        mut self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        // All externally tagged enums are maps with a single entry.
        let tag = self.read_tag()?;
        if tag != PropertyTag::Map {
            return Err(de::Error::invalid_type(
                de::Unexpected::Other(format!("{:?}", tag).as_str()),
                &"map",
            ));
        }
        let count = self.reader.try_get_u32_le().map_err(Error::from_try_get)?;
        if count != 1 {
            return Err(de::Error::invalid_length(
                usize::try_from(count).map_err(Error::other)?,
                &"1 element",
            ));
        }
        visitor.visit_enum(EnumReader {
            reader: self.reader,
        })
    }
}

type WriteHandler<'a, W> = Box<dyn FnMut(PropertyTag, &mut W) -> Result<(), Error> + 'a>;

struct PropertySerializer<'a, W> {
    write_tag: WriteHandler<'a, W>,
    writer: &'a mut W,
}

impl<'a, W> PropertySerializer<'a, W>
where
    W: Write + Seek,
{
    fn new(writer: &'a mut W) -> Self {
        Self::new_tagged_impl(None, writer)
    }

    fn new_tagged(tag: PropertyTag, writer: &'a mut W) -> Self {
        Self::new_tagged_impl(Some(tag), writer)
    }

    fn new_tagged_impl(tag: Option<PropertyTag>, writer: &'a mut W) -> Self {
        let mut curr_tag = tag;
        Self {
            write_tag: Box::new(move |tag, writer| {
                if let Some(curr_tag) = &curr_tag {
                    if &tag != curr_tag {
                        return Err(Error::InvalidPropertyType { tag: tag.to_u16() });
                    }
                } else {
                    writer.write_u16_le(tag.to_u16())?;
                    curr_tag = Some(tag);
                }

                Ok(())
            }),
            writer,
        }
    }

    fn handle_tag(&mut self, tag: PropertyTag) -> Result<(), Error> {
        (self.write_tag)(tag, self.writer)
    }
}

impl<'b, W> Serializer for &'b mut PropertySerializer<'_, W>
where
    W: Write + Seek,
{
    type Ok = ();

    type Error = Error;

    type SerializeSeq = SeqSerializer<'b, W>;

    type SerializeTuple = SeqSerializer<'b, W>;

    type SerializeTupleStruct = SeqSerializer<'b, W>;

    type SerializeTupleVariant = SeqSerializer<'b, W>;

    type SerializeMap = MapSerializer<'b, W>;

    type SerializeStruct = MapSerializer<'b, W>;

    type SerializeStructVariant = MapSerializer<'b, W>;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        self.handle_tag(PropertyTag::Bool)?;
        self.writer.write_u8(u8::from(v))?;
        Ok(())
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        self.handle_tag(PropertyTag::I8)?;
        self.writer.write_i8(v)?;
        Ok(())
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        self.handle_tag(PropertyTag::I16)?;
        self.writer.write_i16_le(v)?;
        Ok(())
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        self.handle_tag(PropertyTag::I32)?;
        self.writer.write_i32_le(v)?;
        Ok(())
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        self.handle_tag(PropertyTag::I64)?;
        self.writer.write_i64_le(v)?;
        Ok(())
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        self.handle_tag(PropertyTag::U8)?;
        self.writer.write_u8(v)?;
        Ok(())
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        self.handle_tag(PropertyTag::U16)?;
        self.writer.write_u16_le(v)?;
        Ok(())
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        self.handle_tag(PropertyTag::U32)?;
        self.writer.write_u32_le(v)?;
        Ok(())
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        self.handle_tag(PropertyTag::U64)?;
        self.writer.write_u64_le(v)?;
        Ok(())
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        self.handle_tag(PropertyTag::F32)?;
        self.writer.write_f32_le(v)?;
        Ok(())
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        self.handle_tag(PropertyTag::F64)?;
        self.writer.write_f64_le(v)?;
        Ok(())
    }

    fn serialize_char(self, _v: char) -> Result<Self::Ok, Self::Error> {
        Err(Error::unsupported_type("char"))
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        self.handle_tag(PropertyTag::String)?;
        self.writer
            .write_u32_le(u32::try_from(v.len()).map_err(Error::other)?)?;
        self.writer.write_all(v.as_bytes())?;
        Ok(())
    }

    fn serialize_bytes(self, _v: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(Error::unsupported_type("bytes"))
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Err(Error::unsupported_type("none"))
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + ser::Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(Error::unsupported_type("()"))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Err(Error::unsupported_type("unit struct"))
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Err(Error::unsupported_type("unit variant"))
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + ser::Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + ser::Serialize,
    {
        let mut map = self.serialize_map(Some(1))?;
        map.serialize_key(variant)?;
        map.serialize_value(value)?;
        map.end()
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        self.handle_tag(PropertyTag::Vec)?;
        Ok(SeqSerializer {
            serialized_values: Vec::with_capacity(len.unwrap_or(0)),
            writer: self.writer,
        })
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        // Manually write out the header for a single-element map
        self.handle_tag(PropertyTag::Map)?;
        // Exactly one element
        self.writer.write_u32_le(1)?;
        // Write the key
        (&mut PropertySerializer::new_tagged(PropertyTag::String, self.writer))
            .serialize_str(variant)?;
        self.writer.write_u16_le(PropertyTag::Vec.to_u16())?;
        Ok(SeqSerializer {
            serialized_values: Vec::with_capacity(len),
            writer: self.writer,
        })
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        self.handle_tag(PropertyTag::Map)?;
        let length_offset = self.writer.stream_position()?;
        self.writer.write_u32_le(0)?;
        Ok(MapSerializer {
            field_count: 0,
            length_offset,
            writer: self.writer,
        })
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        self.serialize_map(Some(len))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        // Manually write out the header for a single-element map
        self.handle_tag(PropertyTag::Map)?;
        // Exactly one element
        self.writer.write_u32_le(1)?;
        // Write the key
        (&mut PropertySerializer::new_tagged(PropertyTag::String, self.writer))
            .serialize_str(variant)?;
        self.writer.write_u16_le(PropertyTag::Map.to_u16())?;
        let length_offset = self.writer.stream_position()?;
        self.writer.write_u32_le(0)?;

        Ok(MapSerializer {
            field_count: 0,
            length_offset,
            writer: self.writer,
        })
    }
}

enum SeqTagReader {
    None,
    OneTag(PropertyTag),
    MultiTag,
}

struct SeqSerializer<'a, W> {
    serialized_values: Vec<(PropertyTag, Vec<u8>)>,
    writer: &'a mut W,
}

impl<W> ser::SerializeSeq for SeqSerializer<'_, W>
where
    W: Write,
{
    type Ok = ();

    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + ser::Serialize,
    {
        let mut data = io::Cursor::new(Vec::new());

        let written_tag = Cell::new(None);

        {
            let mut serializer = PropertySerializer {
                write_tag: Box::new(|tag, _writer| {
                    written_tag.set(Some(tag));
                    Ok(())
                }),
                writer: &mut data,
            };

            T::serialize(value, &mut serializer)?;
        }

        let tag = written_tag
            .get()
            .expect("Failed to write tag on successful serialization");

        self.serialized_values.push((tag, data.into_inner()));
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let is_homogenous = self
            .serialized_values
            .iter()
            .map(|(tag, _)| tag)
            .all_equal();

        let global_tag = if self.serialized_values.is_empty() || !is_homogenous {
            None
        } else {
            let tag = self.serialized_values[0].0;
            Some(tag)
        };

        let raw_tag = match global_tag {
            Some(tag) => tag.to_u16(),
            None => 0,
        };

        self.writer.write_u32_le(
            u32::try_from(self.serialized_values.len()).map_err(|_| Error::TooManyElements)?,
        )?;
        self.writer.write_u16_le(raw_tag)?;
        for (tag, data) in self.serialized_values {
            if global_tag.is_none() {
                self.writer.write_u16_le(tag.to_u16())?;
            }
            self.writer.write_all(&data)?;
        }
        Ok(())
    }
}

impl<W> ser::SerializeTuple for SeqSerializer<'_, W>
where
    W: Write,
{
    type Ok = ();

    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + ser::Serialize,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        ser::SerializeSeq::end(self)
    }
}

impl<W> ser::SerializeTupleStruct for SeqSerializer<'_, W>
where
    W: Write,
{
    type Ok = ();

    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + ser::Serialize,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        ser::SerializeSeq::end(self)
    }
}

impl<W> ser::SerializeTupleVariant for SeqSerializer<'_, W>
where
    W: Write,
{
    type Ok = ();

    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + ser::Serialize,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        ser::SerializeSeq::end(self)
    }
}

struct MapSerializer<'a, W> {
    field_count: u32,
    length_offset: u64,
    writer: &'a mut W,
}

impl<W> ser::SerializeMap for MapSerializer<'_, W>
where
    W: Write + Seek,
{
    type Ok = ();

    type Error = Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + ser::Serialize,
    {
        self.field_count += 1;
        key.serialize(&mut PropertySerializer::new_tagged(
            PropertyTag::String,
            self.writer,
        ))
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + ser::Serialize,
    {
        value.serialize(&mut PropertySerializer::new(self.writer))
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let curr_pos = self.writer.stream_position()?;
        self.writer.seek(SeekFrom::Start(self.length_offset))?;
        self.writer.write_u32_le(self.field_count)?;
        self.writer.seek(SeekFrom::Start(curr_pos))?;
        Ok(())
    }
}

impl<W> ser::SerializeStruct for MapSerializer<'_, W>
where
    W: Write + Seek,
{
    type Ok = ();

    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + ser::Serialize,
    {
        self.serialize_key(key)?;
        self.serialize_value(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        ser::SerializeMap::end(self)
    }
}

impl<W> ser::SerializeStructVariant for MapSerializer<'_, W>
where
    W: Write + Seek,
{
    type Ok = ();

    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + ser::Serialize,
    {
        self.serialize_key(key)?;
        self.serialize_value(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        ser::SerializeMap::end(self)
    }
}

pub(crate) fn read_from_prop_map<'de, T>(data: &'de [u8]) -> Result<T, Error>
where
    T: de::Deserialize<'de>,
{
    let mut reader = data;
    let deserializer = PropertyDeserializer::new_tagged(PropertyTag::Map, &mut reader);
    T::deserialize(deserializer)
}

pub(crate) fn serialize_prop_map<T, W>(value: &T, writer: &mut W) -> Result<(), Error>
where
    T: ?Sized + ser::Serialize,
    W: Write + Seek,
{
    value.serialize(&mut PropertySerializer::new_tagged(
        PropertyTag::Map,
        writer,
    ))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct SimpleStruct {
        integer: i32,
        float: f64,
        boolean: bool,
        text: String,
    }

    #[test]
    fn test_round_trip_simple_struct() {
        let input = SimpleStruct {
            integer: 42,
            float: std::f64::consts::PI,
            boolean: true,
            text: "Hello, world!".to_string(),
        };

        let mut buffer = std::io::Cursor::new(Vec::new());
        serialize_prop_map(&input, &mut buffer).unwrap();

        let data = buffer.into_inner();
        let output: SimpleStruct = read_from_prop_map(&data).unwrap();

        assert_eq!(input, output);
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct NestedStruct {
        id: u32,
        meta: SimpleStruct,
    }

    #[test]
    fn test_round_trip_nested_struct() {
        let input = NestedStruct {
            id: 1001,
            meta: SimpleStruct {
                integer: 7,
                float: 0.0,
                boolean: false,
                text: "Nested".to_string(),
            },
        };

        let mut buffer = std::io::Cursor::new(Vec::new());
        serialize_prop_map(&input, &mut buffer).unwrap();

        let data = buffer.into_inner();
        let output: NestedStruct = read_from_prop_map(&data).unwrap();

        assert_eq!(input, output);
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct StructWithVec {
        numbers: Vec<i32>,
        names: Vec<String>,
    }

    #[test]
    fn test_round_trip_struct_with_vec() {
        let input = StructWithVec {
            numbers: vec![1, 2, 3, 5, 8],
            names: vec!["Alice".to_string(), "Bob".to_string()],
        };

        let mut buffer = std::io::Cursor::new(Vec::new());
        serialize_prop_map(&input, &mut buffer).unwrap();

        let data = buffer.into_inner();
        let output: StructWithVec = read_from_prop_map(&data).unwrap();

        assert_eq!(input, output);
    }

    #[test]
    fn test_round_trip_hashmap() {
        let mut input = HashMap::new();
        input.insert("key1".to_string(), 123);
        input.insert("key2".to_string(), 456);

        let mut buffer = std::io::Cursor::new(Vec::new());
        serialize_prop_map(&input, &mut buffer).unwrap();

        let data = buffer.into_inner();

        let output: HashMap<String, i32> = read_from_prop_map(&data).unwrap();

        assert_eq!(input, output);
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    enum TestEnum {
        Tuple(i32, String),
        Struct { x: i32, y: i32 },
    }

    #[test]
    fn test_round_trip_enum_tuple() {
        let input = TestEnum::Tuple(42, "Answer".to_string());

        let mut buffer = std::io::Cursor::new(Vec::new());
        serialize_prop_map(&input, &mut buffer).unwrap();

        let data = buffer.into_inner();
        let output: TestEnum = read_from_prop_map(&data).unwrap();

        assert_eq!(input, output);
    }

    #[test]
    fn test_round_trip_enum_struct() {
        let input = TestEnum::Struct { x: 10, y: 20 };

        let mut buffer = std::io::Cursor::new(Vec::new());
        serialize_prop_map(&input, &mut buffer).unwrap();

        let data = buffer.into_inner();
        let output: TestEnum = read_from_prop_map(&data).unwrap();

        assert_eq!(input, output);
    }
}
