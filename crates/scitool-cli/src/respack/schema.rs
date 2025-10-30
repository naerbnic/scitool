mod base64;
mod res_id;
mod sha256_hash;

use std::{fmt::Debug, io};

use scidev::resources::ResourceId;
use serde::{Deserialize, Serialize};

pub(super) use self::{base64::Base64Data, sha256_hash::Sha256Hash};

pub(super) const CURRENT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedInfo {
    buffer: BufferInfo,
}

impl CompressedInfo {
    #[must_use]
    pub(crate) fn new(buffer: BufferInfo) -> Self {
        Self { buffer }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// The volume source info of a resource (i.e. inside of a resource file, like `RESOURCES.000`).
pub struct VolumeSource {
    /// The number of the archive file containing the resource. For example, 0 for `RESOURCES.000`.
    archive_num: u16,
    /// The offset within the archive file where the resource data starts.
    archive_offset: u32,
}

impl VolumeSource {
    #[must_use]
    pub(crate) fn new(archive_num: u16, archive_offset: u32) -> Self {
        Self {
            archive_num,
            archive_offset,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum HeaderData {
    Simple(Base64Data),
    Composite {
        ext_header_data: Base64Data,
        extra_data: Base64Data,
    },
}

/// The patch source info of a resource (i.e. a separate patch file, like `12.spr`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchSource {
    /// The initial data of the header of the patch file, before the actual resource data.
    patch_header_data: HeaderData,
}

impl PatchSource {
    #[must_use]
    pub(crate) fn new(patch_header_data: HeaderData) -> Self {
        PatchSource { patch_header_data }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "source")]
pub enum SourceInfo {
    Volume(VolumeSource),
    Patch(PatchSource),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferInfo {
    /// The size of the buffer in bytes.
    size: u64,
    /// The SHA-256 hash of the buffer data.
    hash: Sha256Hash,
}

impl BufferInfo {
    #[must_use]
    pub(crate) fn new(size: u64, hash: Sha256Hash) -> Self {
        Self { size, hash }
    }

    pub(crate) fn from_stream<R: std::io::Read>(reader: R) -> io::Result<Self> {
        let (hash, size) = Sha256Hash::from_stream_hash(reader)?;
        Ok(BufferInfo { size, hash })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentInfo {
    /// Info about the raw bytes.
    pub(super) raw: BufferInfo,

    /// Info about the original raw bytes, if this was exported from an
    /// existing resource package.
    pub(super) original_raw: Option<BufferInfo>,

    // If the data was compressed, information about the compressed data.
    pub(super) compressed: Option<CompressedInfo>,
}

/// Metadata about a resource in a project. This is general for all types of
/// resources.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    /// The version of the metadata schema. This can be used to handle
    /// different versions of the schema in the future.
    pub(super) version: u32,

    /// The type of the resource.
    #[serde(with = "self::res_id::ResourceIdSerde")]
    pub(super) id: ResourceId,

    /// Information about the content of the resource, if available.
    pub(super) content: Option<ContentInfo>,

    /// Info about the source of the resource data. Can be used to help round-trip the data.
    pub(super) source: Option<SourceInfo>,
}

impl Metadata {
    /// Create a new `Metadata` instance that was not loaded from a package.
    #[must_use]
    pub fn new_with_id(id: ResourceId) -> Self {
        Metadata {
            version: CURRENT_VERSION,
            id,
            content: None,
            source: None,
        }
    }

    /// Get the resource ID of the resource.
    #[must_use]
    pub fn resource_id(&self) -> ResourceId {
        self.id
    }

    pub fn set_resource_id(&mut self, id: ResourceId) {
        self.id = id;
    }

    pub(crate) fn set_raw_data_info(&mut self, buffer_info: BufferInfo) {
        if let Some(content) = &mut self.content {
            content.raw = buffer_info;
        } else {
            self.content = Some(ContentInfo {
                raw: buffer_info,
                original_raw: None,
                compressed: None,
            });
        }
    }
}
