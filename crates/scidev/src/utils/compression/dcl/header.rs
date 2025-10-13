use std::io;

use crate::utils::compression::{reader::BitReader, writer::BitWriter};

#[derive(Debug, thiserror::Error)]
#[error("Header decode error: {message}")]
pub struct DecodeError {
    message: String,
    #[source]
    source: Option<io::Error>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionMode {
    Ascii,
    Binary,
}

impl CompressionMode {
    pub(super) fn write_to<W: BitWriter>(self, writer: &mut W) -> io::Result<()> {
        writer.write_u8(self.into())?;
        Ok(())
    }
}

impl From<CompressionMode> for u8 {
    fn from(mode: CompressionMode) -> Self {
        match mode {
            CompressionMode::Ascii => 1,
            CompressionMode::Binary => 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DictType {
    Size1024 = 4,
    Size2048 = 5,
    Size4096 = 6,
}

impl DictType {
    pub(super) fn write_to<W: BitWriter>(self, writer: &mut W) -> io::Result<()> {
        writer.write_u8(self.into())?;
        Ok(())
    }
}

impl DictType {
    #[must_use]
    pub fn dict_size(self) -> usize {
        match self {
            DictType::Size1024 => 1024,
            DictType::Size2048 => 2048,
            DictType::Size4096 => 4096,
        }
    }

    #[must_use]
    pub fn num_extra_bits(self) -> u8 {
        match self {
            DictType::Size1024 => 4,
            DictType::Size2048 => 5,
            DictType::Size4096 => 6,
        }
    }
}

impl From<DictType> for u8 {
    fn from(dict_type: DictType) -> Self {
        dict_type as u8
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct CompressionHeader {
    mode: CompressionMode,
    dict_type: DictType,
}

impl CompressionHeader {
    pub(super) fn from_bits<R: BitReader>(reader: &mut R) -> Result<Self, DecodeError> {
        let mode = reader.read_u8().map_err(|e| DecodeError {
            message: "Failed to read DCL mode".into(),
            source: Some(e),
        })?;
        let dict_type = reader.read_u8().map_err(|e| DecodeError {
            message: "Failed to read DCL dictionary type".into(),
            source: Some(e),
        })?;

        let mode = match mode {
            0 => CompressionMode::Binary,
            1 => CompressionMode::Ascii,
            _ => {
                return Err(DecodeError {
                    message: format!("Unsupported DCL mode: {mode}"),
                    source: None,
                });
            }
        };

        let dict_type = match dict_type {
            4 => DictType::Size1024,
            5 => DictType::Size2048,
            6 => DictType::Size4096,
            _ => {
                return Err(DecodeError {
                    message: format!("Unsupported DCL dictionary type: {dict_type}"),
                    source: None,
                });
            }
        };

        Ok(CompressionHeader { mode, dict_type })
    }

    pub(super) fn mode(self) -> CompressionMode {
        self.mode
    }

    pub(super) fn dict_type(self) -> DictType {
        self.dict_type
    }
}
