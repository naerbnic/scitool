//! `scires` package directories.
//!
//! A *.scires package is a folder that has a `meta.json` file and multiple files in it, to
//! be able to create a common workable format for importing and exporting SCI resources.
mod dirty;
mod err_helpers;
mod io_wrappers;
pub mod schema;

use std::{
    borrow::Cow,
    io::{self, Read as _},
    path::Path,
};

use atomic_dir::{AtomicDir, CreateMode, UpdateBuilder, UpdateInitMode};
use scidev::{
    resources::{ExtraData, Resource, ResourceId, ResourceProvenance},
    utils::{block::Block, compression::dcl::DecompressFactory},
};

use crate::respack::{
    err_helpers::{io_bail, io_err_map},
    io_wrappers::LengthLimitedReader,
    schema::{BufferInfo, Sha256Hash},
};

use self::{dirty::Dirty, schema::Metadata};

const META_PATH: &str = "meta.json";
const COMPRESSED_BIN_PATH: &str = "compressed.bin";
const RAW_BIN_PATH: &str = "raw.bin";

fn buffer_info_from_lazy_block(block: &Block) -> io::Result<schema::BufferInfo> {
    let buffer = block.open_mem(..)?;

    let size = u64::try_from(buffer.len()).unwrap();
    let hash = Sha256Hash::from_data_hash(&*buffer);

    Ok(schema::BufferInfo::new(size, hash))
}

pub struct ResPack {
    metadata: Dirty<Metadata>,
    compressed_data: Dirty<Option<Block>>,
    raw_data: Dirty<Option<Block>>,
    disk_backing: Option<AtomicDir>,
}

impl ResPack {
    #[must_use]
    pub fn new(id: ResourceId) -> Self {
        ResPack {
            metadata: Dirty::new_fresh(Metadata::new_with_id(id)),
            compressed_data: Dirty::new_fresh(None),
            raw_data: Dirty::new_fresh(None),
            disk_backing: None,
        }
    }

    pub fn from_resource(resource: &Resource) -> io::Result<Self> {
        let raw_data = resource.data().clone();

        let raw_buffer_info = BufferInfo::from_stream(raw_data.open_reader(..)?)?;

        // Get provenance info from the contents.
        let source = match resource.provenance() {
            ResourceProvenance::Volume(volume_source) => {
                Some(schema::SourceInfo::Volume(schema::VolumeSource::new(
                    volume_source.archive_num(),
                    volume_source.archive_offset(),
                )))
            }
            ResourceProvenance::PatchFile(patch_source) => {
                let header_data = match patch_source.extra_data() {
                    ExtraData::Simple(data) => {
                        let data = data.open_mem(..)?;
                        schema::HeaderData::Simple(schema::Base64Data::new(data.to_vec()))
                    }
                    ExtraData::Composite {
                        ext_header,
                        extra_data,
                    } => {
                        let ext_header = ext_header.open_mem(..)?;
                        let extra_data = extra_data.open_mem(..)?;
                        schema::HeaderData::Composite {
                            ext_header_data: schema::Base64Data::new(ext_header.to_vec()),
                            extra_data: schema::Base64Data::new(extra_data.to_vec()),
                        }
                    }
                };
                Some(schema::SourceInfo::Patch(schema::PatchSource::new(
                    header_data,
                )))
            }
            ResourceProvenance::New => None,
        };

        let (compressed_info, compressed_data) =
            if let Some(compressed) = resource.compressed() {
                let block = compressed.compressed_block().clone();
                let compressed_info = BufferInfo::from_stream(
                    block
                        .open_reader(..)
                        .map_err(io_err_map!(Other, "Failed to open compressed data"))?,
                )?;
                (
                    Some(schema::CompressedInfo::new(compressed_info)),
                    Some(block),
                )
            } else {
                (None, None)
            };

        let metadata = Metadata {
            version: schema::CURRENT_VERSION,
            id: *resource.id(),
            content: Some(schema::ContentInfo {
                raw: raw_buffer_info.clone(),
                original_raw: Some(raw_buffer_info.clone()),
                compressed: compressed_info,
            }),
            source,
        };

        Ok(ResPack {
            metadata: Dirty::new_stored(metadata),
            compressed_data: Dirty::new_stored(compressed_data),
            raw_data: Dirty::new_stored(Some(raw_data)),
            disk_backing: None,
        })
    }

    pub fn load_from_path<'a, P>(path: P) -> std::io::Result<Self>
    where
        P: Into<Cow<'a, Path>>,
    {
        let base_path = path.into().into_owned();

        let dir = AtomicDir::open(&base_path).map_err(io_err_map!(
            NotFound,
            "Failed to open package directory at {}",
            base_path.display()
        ))?;

        let mut meta_file = LengthLimitedReader::new(
            dir.open_file(META_PATH).map_err(io_err_map!(
                NotFound,
                "Failed to open metadata file at {}",
                base_path.join(META_PATH).display()
            ))?,
            128 * 1024, // 128 KiB
        );

        let mut data = Vec::new();
        meta_file.read_to_end(&mut data)?;
        let metadata: Metadata = serde_json::from_slice(&data)
            .map_err(io_err_map!(InvalidData, "Failed to parse metadata JSON"))?;

        let compressed_path = base_path.join(COMPRESSED_BIN_PATH);
        let compressed_data = if std::fs::exists(&compressed_path)? {
            let block_source = Block::from_path(compressed_path)
                .map_err(io_err_map!(Other, "Failed to create block source"))?;
            Some(block_source)
        } else {
            None
        };

        let raw_data_path = base_path.join(RAW_BIN_PATH);
        let raw_data = if std::fs::exists(&raw_data_path)? {
            let block_source = Block::from_path(raw_data_path)
                .map_err(io_err_map!(Other, "Failed to create block source"))?;
            Some(block_source)
        } else {
            compressed_data
                .as_ref()
                .map(|compressed_data| {
                    Block::builder()
                        .build_from_read_factory(DecompressFactory::new(compressed_data.clone()))
                })
                .transpose()
                .map_err(io_err_map!(Other, "Failed to decompress data"))?
        };

        Ok(Self {
            metadata: Dirty::new_stored(metadata),
            compressed_data: Dirty::new_stored(compressed_data),
            raw_data: Dirty::new_stored(raw_data),
            disk_backing: Some(dir),
        })
    }

    #[must_use]
    pub fn resource_id(&self) -> ResourceId {
        self.metadata().resource_id()
    }

    pub fn set_resource_id(&mut self, id: ResourceId) {
        self.metadata_mut().set_resource_id(id);
    }

    #[must_use]
    pub fn metadata(&self) -> &Metadata {
        self.metadata.get()
    }

    pub fn metadata_mut(&mut self) -> &mut Metadata {
        self.metadata.get_mut()
    }

    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.metadata.is_dirty() || self.raw_data.is_dirty() || self.compressed_data.is_dirty()
    }

    pub fn set_raw_data(&mut self, data: Block) -> std::io::Result<()> {
        // Update the metadata about the raw data.
        let raw_buffer_info = buffer_info_from_lazy_block(&data)
            .map_err(io_err_map!(Other, "Failed to compute buffer info"))?;
        {
            let metadata = self.metadata_mut();
            metadata.set_raw_data_info(raw_buffer_info);
        }
        self.raw_data.set(Some(data));
        Ok(())
    }

    pub fn save(&mut self) -> std::io::Result<()> {
        let Some(dir) = self.disk_backing.take() else {
            io_bail!(
                InvalidInput,
                "Cannot save a package that was not loaded from a path or saved to a path."
            );
        };

        let atomic_dir = dir.begin_update(UpdateInitMode::CopyExisting)?;

        if self.metadata.is_dirty() {
            let meta_json = serde_json::to_vec(self.metadata.get())?;
            atomic_dir.write_file(META_PATH, CreateMode::Overwrite, &meta_json)?;
        }

        if self.compressed_data.is_dirty() {
            if let Some(compressed_data) = self.compressed_data.get() {
                let data = compressed_data
                    .open_mem(..)
                    .map_err(io_err_map!(Other, "Failed to open compressed data"))?;

                atomic_dir.write_file(COMPRESSED_BIN_PATH, CreateMode::Overwrite, &data)?;
            } else if atomic_dir.exists(COMPRESSED_BIN_PATH)? {
                atomic_dir.remove_file(COMPRESSED_BIN_PATH)?;
            }
        }

        if self.raw_data.is_dirty() {
            if let Some(raw_data) = self.raw_data.get() {
                let data = raw_data
                    .open_mem(..)
                    .map_err(io_err_map!(Other, "Failed to open raw data"))?;

                atomic_dir.write_file(RAW_BIN_PATH, CreateMode::Overwrite, &data)?;
            } else if atomic_dir.exists(RAW_BIN_PATH)? {
                atomic_dir.remove_file(RAW_BIN_PATH)?;
            }
        }

        self.disk_backing = Some(atomic_dir.commit()?);
        self.metadata.mark_clean();
        self.compressed_data.mark_clean();
        self.raw_data.mark_clean();

        Ok(())
    }

    /// Saves the package to a new path.
    ///
    /// This will update the stored path of the package to the new path, ensuring
    /// all files are saved there. If this was previously loaded from a path,
    /// the previous files will not be modified, but the old path will be forgotten.
    pub fn save_to<P>(&mut self, path: &P) -> std::io::Result<()>
    where
        P: AsRef<Path> + ?Sized,
    {
        let atomic_dir = UpdateBuilder::open_at(path, UpdateInitMode::LeaveEmpty)?;

        atomic_dir.write_file(
            META_PATH,
            CreateMode::Overwrite,
            &serde_json::to_vec(self.metadata.get())?,
        )?;

        if let Some(compressed_data) = self.compressed_data.get() {
            let data = compressed_data
                .open_mem(..)
                .map_err(io_err_map!(Other, "Failed to open compressed data"))?;

            atomic_dir.write_file(COMPRESSED_BIN_PATH, CreateMode::Overwrite, &data)?;
        }

        if let Some(raw_data) = self.raw_data.get() {
            let data = raw_data
                .open_mem(..)
                .map_err(io_err_map!(Other, "Failed to open raw data"))?;

            atomic_dir.write_file(RAW_BIN_PATH, CreateMode::Overwrite, &data)?;
        }

        self.disk_backing = Some(atomic_dir.commit()?);
        self.metadata.mark_clean();
        self.raw_data.mark_clean();
        self.compressed_data.mark_clean();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{META_PATH, RAW_BIN_PATH, ResPack, schema::Sha256Hash};
    use bytes::Bytes;
    use scidev::{
        resources::{ResourceId, ResourceType},
        utils::block::{Block, MemBlock},
    };
    use serde_json::Value;
    use tempfile::tempdir;

    #[test]
    fn set_raw_data_updates_metadata_snapshot() -> std::io::Result<()> {
        let id = ResourceId::new(ResourceType::Script, 123);
        let mut package = ResPack::new(id);

        let raw_bytes = b"hello sci".to_vec();
        let block = Block::from_mem_block(MemBlock::from_vec(raw_bytes.clone()));
        package.set_raw_data(block)?;

        let metadata_value =
            serde_json::to_value(package.metadata()).expect("metadata should serialize to JSON");
        let content = metadata_value
            .get("content")
            .and_then(Value::as_object)
            .expect("content section missing after set_raw_data");
        let raw_entry = content
            .get("raw")
            .and_then(Value::as_object)
            .expect("raw buffer info missing");

        assert_eq!(
            raw_entry.get("size").and_then(Value::as_u64),
            Some(raw_bytes.len() as u64),
            "raw size should reflect input block",
        );

        let expected_hash = Sha256Hash::from_data_hash(Bytes::from(raw_bytes.clone()));
        let expected_hash_value =
            serde_json::to_value(&expected_hash).expect("hash should serialize to JSON value");
        let expected_hash_str = expected_hash_value
            .as_str()
            .expect("serialized hash should be a string");

        assert_eq!(
            raw_entry.get("hash").and_then(Value::as_str),
            Some(expected_hash_str),
            "raw hash should match computed hash",
        );

        Ok(())
    }

    #[test]
    fn save_to_writes_files_and_allows_followup_save() -> std::io::Result<()> {
        let temp_dir = tempdir()?;
        let package_dir = temp_dir.path().join("pkg");

        let mut package = ResPack::new(ResourceId::new(ResourceType::Script, 7));
        let raw_bytes = b"resource data".to_vec();
        let block = Block::from_mem_block(MemBlock::from_vec(raw_bytes.clone()));
        package.set_raw_data(block)?;

        package.save_to(&package_dir)?;

        assert!(
            std::fs::exists(package_dir.join(META_PATH))?,
            "meta.json should exist after save_to"
        );

        assert!(
            std::fs::exists(package_dir.join(RAW_BIN_PATH))?,
            "raw.bin should exist after save_to"
        );

        let meta_bytes = std::fs::read(package_dir.join(META_PATH))?;
        let meta_json: Value = serde_json::from_slice(&meta_bytes)
            .expect("metadata should deserialize into JSON value");

        let raw_entry = meta_json
            .get("content")
            .and_then(|c| c.get("raw"))
            .and_then(Value::as_object)
            .expect("raw content section missing after save_to");
        assert_eq!(
            raw_entry.get("size").and_then(Value::as_u64),
            Some(raw_bytes.len() as u64)
        );

        // Save again to ensure the package path was remembered and the transaction is reusable.
        package.save()?;

        ResPack::load_from_path(&package_dir).expect("package should reload from saved directory");

        Ok(())
    }
}
