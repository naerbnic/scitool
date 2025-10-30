use base64::Engine as _;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub(crate) struct Base64Data(Vec<u8>);

impl Base64Data {
    #[must_use]
    pub(crate) fn new(data: Vec<u8>) -> Self {
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
