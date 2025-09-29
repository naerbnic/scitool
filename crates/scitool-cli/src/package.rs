//! `scires` package directories.
//!
//! A *.scires package is a folder that has a `meta.json` file and multiple files in it, to
//! be able to create a common workable format for importing and exporting SCI resources.
mod dirty;
pub mod schema;

use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

use scidev::{
    resources::ResourceId,
    utils::{
        block::{BlockSource, LazyBlock, LazyBlockError},
        compression::dcl::decompress_dcl,
    },
};

use tokio::io::AsyncReadExt as _;

use crate::{
    fs::{
        atomic_dir::AtomicDir,
        err_helpers::{io_bail, io_err_map},
        io_wrappers::LengthLimitedAsyncReader,
        ops::WriteMode,
    },
    package::schema::Sha256Hash,
};

use self::{dirty::Dirty, schema::Metadata};

const META_PATH: &str = "meta.json";
const COMPRESSED_BIN_PATH: &str = "compressed.bin";
const RAW_BIN_PATH: &str = "raw.bin";

async fn buffer_info_from_lazy_block(
    block: &LazyBlock<'_>,
) -> Result<schema::BufferInfo, LazyBlockError> {
    let buffer = block.open().await?;

    let size = u64::try_from(buffer.len()).unwrap();
    let hash = Sha256Hash::from_data_hash(&*buffer);

    Ok(schema::BufferInfo::new(size, hash))
}

async fn new_block_source_from_atomic_dir(
    atomic_dir: &AtomicDir,
    path: impl AsRef<Path>,
) -> std::io::Result<BlockSource<'static>> {
    let metadata = atomic_dir.metadata(path.as_ref()).await?;
    if !metadata.file_type().is_file() {
        io_bail!(
            InvalidInput,
            "Path is not a file: {}",
            path.as_ref().display()
        );
    }
    let handle = atomic_dir.as_read_only_handle();
    let path = path.as_ref().to_owned();
    BlockSource::from_reader_thunk(
        move || {
            let handle = handle.clone();
            let path = path.clone();
            async move { Ok(handle.open(&path).await?) }
        },
        metadata.len(),
    )
    .map_err(io_err_map!(Other, "Failed to create block source"))
}

struct DirectoryInfo {
    base_path: PathBuf,
    atomic_dir: Option<AtomicDir>,
}

pub struct Package<'a> {
    dir_info: Option<DirectoryInfo>,
    metadata: Dirty<Metadata>,
    compressed_data: Dirty<Option<LazyBlock<'a>>>,
    raw_data: Dirty<Option<LazyBlock<'a>>>,
}

impl<'a> Package<'a> {
    #[must_use]
    pub fn new(id: ResourceId) -> Self {
        Package {
            dir_info: None,
            metadata: Dirty::new_fresh(Metadata::new_with_id(id)),
            compressed_data: Dirty::new_fresh(None),
            raw_data: Dirty::new_fresh(None),
        }
    }

    pub async fn load_from_path<P>(path: P) -> std::io::Result<Self>
    where
        P: Into<Cow<'a, Path>>,
    {
        let base_path = path.into().into_owned();

        let atomic_dir = AtomicDir::new_at_dir(&base_path).await?;

        let mut meta_file = LengthLimitedAsyncReader::new(
            atomic_dir.open_options().read(true).open(META_PATH).await?,
            128 * 1024, // 128 KiB
        );

        let mut data = Vec::new();
        meta_file.read_buf(&mut data).await?;
        let metadata: Metadata = serde_json::from_slice(&data)
            .map_err(io_err_map!(InvalidData, "Failed to parse metadata JSON"))?;

        let compressed_data = if atomic_dir.exists(COMPRESSED_BIN_PATH).await? {
            let block_source = new_block_source_from_atomic_dir(&atomic_dir, COMPRESSED_BIN_PATH)
                .await
                .map_err(io_err_map!(Other, "Failed to create block source"))?;
            Some(LazyBlock::from_block_source(block_source))
        } else {
            None
        };

        let raw_data = if atomic_dir.exists(RAW_BIN_PATH).await? {
            let block_source = new_block_source_from_atomic_dir(&atomic_dir, RAW_BIN_PATH)
                .await
                .map_err(io_err_map!(Other, "Failed to create block source"))?;
            Some(LazyBlock::from_block_source(block_source))
        } else {
            compressed_data.as_ref().map(|compressed_data| {
                compressed_data
                    .clone()
                    .map(|block| decompress_dcl(&block).map_err(LazyBlockError::from_other))
            })
        };

        Ok(Self {
            dir_info: Some(DirectoryInfo {
                base_path,
                atomic_dir: Some(atomic_dir),
            }),
            metadata: Dirty::new_stored(metadata),
            compressed_data: Dirty::new_stored(compressed_data),
            raw_data: Dirty::new_stored(raw_data),
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

    pub async fn set_raw_data(&mut self, data: LazyBlock<'a>) -> std::io::Result<()> {
        // Update the metadata about the raw data.
        let raw_buffer_info = buffer_info_from_lazy_block(&data)
            .await
            .map_err(io_err_map!(Other, "Failed to compute buffer info"))?;
        {
            let metadata = self.metadata_mut();
            metadata.set_raw_data_info(raw_buffer_info);
        }
        self.raw_data.set(Some(data));
        Ok(())
    }

    pub async fn save(&mut self) -> std::io::Result<()> {
        let Some(dir_info) = &mut self.dir_info else {
            io_bail!(
                InvalidInput,
                "Cannot save a package that was not loaded from a path or saved to a path."
            );
        };

        let atomic_dir = if let Some(atomic_dir) = dir_info.atomic_dir.take() {
            atomic_dir
        } else {
            AtomicDir::new_at_dir(&dir_info.base_path).await?
        };

        if self.metadata.is_dirty() {
            let meta_json = serde_json::to_vec(self.metadata.get())
                .map_err(io_err_map!(Other, "Failed to serialize metadata to JSON"))?;
            atomic_dir
                .write(META_PATH, WriteMode::Overwrite, &meta_json)
                .await
                .map_err(io_err_map!(Other, "Failed to write metadata file"))?;
            self.metadata.mark_clean();
        }

        if self.compressed_data.is_dirty() {
            if let Some(compressed_data) = self.compressed_data.get() {
                let data = compressed_data
                    .open()
                    .await
                    .map_err(io_err_map!(Other, "Failed to open compressed data"))?;

                atomic_dir
                    .write(COMPRESSED_BIN_PATH, WriteMode::Overwrite, &data)
                    .await
                    .map_err(io_err_map!(Other, "Failed to write compressed data file"))?;
            } else if atomic_dir.exists(COMPRESSED_BIN_PATH).await? {
                atomic_dir
                    .delete(COMPRESSED_BIN_PATH)
                    .await
                    .map_err(io_err_map!(Other, "Failed to remove compressed data file"))?;
            }
            self.compressed_data.mark_clean();
        }

        if self.raw_data.is_dirty() {
            if let Some(raw_data) = self.raw_data.get() {
                let data = raw_data
                    .open()
                    .await
                    .map_err(io_err_map!(Other, "Failed to open raw data"))?;

                atomic_dir
                    .write(RAW_BIN_PATH, WriteMode::Overwrite, &data)
                    .await
                    .map_err(io_err_map!(Other, "Failed to write raw data file"))?;
            } else if atomic_dir.exists(RAW_BIN_PATH).await? {
                atomic_dir
                    .delete(RAW_BIN_PATH)
                    .await
                    .map_err(io_err_map!(Other, "Failed to remove raw data file"))?;
            }
            self.raw_data.mark_clean();
        }

        atomic_dir.commit().await?;

        Ok(())
    }

    /// Saves the package to a new path.
    ///
    /// This will update the stored path of the package to the new path, ensuring
    /// all files are saved there. If this was previously loaded from a path,
    /// the previous files will not be modified, but the old path will be forgotten.
    pub async fn save_to(&mut self, path: PathBuf) -> std::io::Result<()> {
        let atomic_dir = AtomicDir::new_at_dir(&path).await?;

        let meta_json = serde_json::to_vec(self.metadata.get())
            .map_err(io_err_map!(Other, "Failed to serialize metadata to JSON"))?;
        atomic_dir
            .write(META_PATH, WriteMode::Overwrite, &meta_json)
            .await
            .map_err(io_err_map!(Other, "Failed to write metadata file"))?;
        self.metadata.mark_clean();

        if let Some(compressed_data) = self.compressed_data.get() {
            let data = compressed_data
                .open()
                .await
                .map_err(io_err_map!(Other, "Failed to open compressed data"))?;

            atomic_dir
                .write(COMPRESSED_BIN_PATH, WriteMode::Overwrite, &data)
                .await
                .map_err(io_err_map!(Other, "Failed to write compressed data file"))?;
        }

        if let Some(raw_data) = self.raw_data.get() {
            let data = raw_data
                .open()
                .await
                .map_err(io_err_map!(Other, "Failed to open raw data"))?;

            atomic_dir
                .write(RAW_BIN_PATH, WriteMode::Overwrite, &data)
                .await
                .map_err(io_err_map!(Other, "Failed to write raw data file"))?;
        }
        self.raw_data.mark_clean();

        Ok(())
    }
}
