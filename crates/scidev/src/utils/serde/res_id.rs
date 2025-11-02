use crate::resources::{ResourceId, ResourceType};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Serialize, Deserialize)]
struct RawSerialization<'a> {
    #[serde(rename = "type")]
    resource_type: &'a str,
    #[serde(rename = "num")]
    resource_num: u16,
}

pub struct ResourceIdSerde;

impl ResourceIdSerde {
    pub fn serialize<S>(res_id: &ResourceId, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let raw = RawSerialization {
            resource_type: res_id.type_id().to_file_ext(),
            resource_num: res_id.resource_num(),
        };
        Serialize::serialize(&raw, serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<ResourceId, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw: RawSerialization = Deserialize::deserialize(deserializer)?;
        let res_type =
            ResourceType::from_file_ext(raw.resource_type).map_err(serde::de::Error::custom)?;
        Ok(ResourceId::new(res_type, raw.resource_num))
    }
}
