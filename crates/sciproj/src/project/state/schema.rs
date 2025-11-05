use scidev::{
    resources::ResourceId,
    utils::serde::{ResourceIdSerde, Sha256Hash},
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
struct FileMapping {
    #[serde(with = "ResourceIdSerde")]
    resource_id: ResourceId,
}

#[derive(Serialize, Deserialize, Debug)]
struct KnownFile {
    path: String,
    hash: Sha256Hash,
    size: u64,

    #[serde(flatten)]
    resource_mapping: FileMapping,
}

#[derive(Debug, Default)]
struct KnownFiles(Vec<KnownFile>);

impl Serialize for KnownFiles {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut files: Vec<&KnownFile> = self.0.iter().collect();
        files.sort_by_key(|file| &file.hash);
        files.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for KnownFiles {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let files: Vec<KnownFile> = Deserialize::deserialize(deserializer)?;
        Ok(KnownFiles(files))
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub(super) struct Contents {
    known_files: KnownFiles,
}

impl Default for Contents {
    fn default() -> Self {
        Self {
            known_files: KnownFiles(Vec::new()),
        }
    }
}
