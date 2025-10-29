use bytes::Buf;
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, io};

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Sha256Hash([u8; 32]);

impl Sha256Hash {
    pub(crate) fn from_stream_hash<R: std::io::Read>(mut reader: R) -> io::Result<(Self, u64)> {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        let size = std::io::copy(&mut reader, &mut hasher)?;
        Ok((Sha256Hash(hasher.finalize().into()), size))
    }

    pub(crate) fn from_data_hash<B: Buf>(mut data: B) -> Self {
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
