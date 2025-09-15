mod res_type;

use std::fmt::Debug;

use base64::Engine as _;
use bytes::Buf;
use serde::{Deserialize, Serialize};

use crate::resources::ResourceType;

struct ResourceTypeSerde;

impl ResourceTypeSerde {
    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "Required by serde with attribute"
    )]
    fn serialize<S>(res_type: &ResourceType, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(res_type.to_file_ext())
    }

    fn deserialize<'de, D>(deserializer: D) -> Result<ResourceType, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let ext: &'de str = serde::Deserialize::deserialize(deserializer)?;
        ResourceType::from_file_ext(ext).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Sha256Hash([u8; 32]);

impl Sha256Hash {
    #[expect(dead_code, reason = "Will use to export data")]
    fn from_data_hash<B: Buf>(mut data: B) -> Self {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        loop {
            let remaining = data.remaining();
            if remaining == 0 {
                break;
            }
            let chunk = data.chunk();
            hasher.update(chunk);
            data.advance(chunk.len());
        }
        Sha256Hash(hasher.finalize().into())
    }

    fn from_hex_str(s: &str) -> Result<Self, hex::FromHexError> {
        let mut arr = [0u8; 32];
        hex::decode_to_slice(s, &mut arr)?;
        Ok(Sha256Hash(arr))
    }

    fn to_hex_string(&self) -> String {
        hex::encode(self.0)
    }
}

impl Serialize for Sha256Hash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_hex_string())
    }
}

impl<'de> Deserialize<'de> for Sha256Hash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s: &'de str = serde::Deserialize::deserialize(deserializer)?;
        Sha256Hash::from_hex_str(s).map_err(serde::de::Error::custom)
    }
}

impl Debug for Sha256Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Sha256Hash")
            .field(&self.to_hex_string())
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct Base64Data(Vec<u8>);

impl Base64Data {
    #[must_use]
    pub fn new(data: Vec<u8>) -> Self {
        Base64Data(data)
    }
}

impl std::ops::Deref for Base64Data {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Serialize for Base64Data {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&base64::engine::general_purpose::STANDARD_NO_PAD.encode(&self.0))
    }
}

impl<'de> Deserialize<'de> for Base64Data {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s: &'de str = serde::Deserialize::deserialize(deserializer)?;
        let data = base64::engine::general_purpose::STANDARD_NO_PAD
            .decode(s)
            .map_err(serde::de::Error::custom)?;
        Ok(Base64Data(data))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    resources: Vec<Resource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedInfo {
    /// Compressed size of the data in bytes.
    compressed_size: u32,
    /// SHA-256 hash of the compressed data.
    compressed_hash: Sha256Hash,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    /// The type of the resource.
    #[serde(with = "ResourceTypeSerde")]
    res_type: ResourceType,
    /// The number of the resource.
    res_num: u16,

    /// Size of the raw resource data in bytes.
    raw_size: u32,
    /// SHA-256 hash of the raw resource data.
    raw_hash: Sha256Hash,
    // If the data was compressed, information about the compressed data.
    compressed: Option<CompressedInfo>,

    /// The number of the archive file containing the resource. For example, 0 for `RESOURCES.000`.
    archive_num: u16,
    /// The offset within the archive file where the resource data starts.
    archive_offset: u32,

    /// If this data was derived from a patch file, this contains any extra header information, encoded in base64.
    patch_header: Option<Base64Data>,

    /// The name of the unpacked raw file, if available. This is relative to the project root.
    unpacked_name: Option<String>,
}
