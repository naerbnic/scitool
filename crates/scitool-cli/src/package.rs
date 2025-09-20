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
    fs::err_helpers::{io_async_bail, io_bail, io_err_map},
    package::schema::Sha256Hash,
};

use self::{dirty::Dirty, schema::Metadata};

const META_PATH: &str = "meta.json";
const COMPRESSED_BIN_PATH: &str = "compressed.bin";
const RAW_BIN_PATH: &str = "raw.bin";

fn buffer_info_from_lazy_block(block: &LazyBlock) -> Result<schema::BufferInfo, LazyBlockError> {
    let buffer = block.open()?;

    let size = u64::try_from(buffer.len()).unwrap();
    let hash = Sha256Hash::from_data_hash(&*buffer);

    Ok(schema::BufferInfo::new(size, hash))
}

/// Provides an upper bound on the data that can be read from a reader. If the
/// data available from the reader exceeds the limit, it will return an
/// [`std::io::ErrorKind::UnexpectedEof`] error.
#[pin_project::pin_project]
struct LengthLimitedAsyncReader<R> {
    #[pin]
    inner: R,
    remaining: u64,
}

impl<R> tokio::io::AsyncRead for LengthLimitedAsyncReader<R>
where
    R: tokio::io::AsyncRead + Unpin,
{
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let initial_remaining = buf.remaining() as u64;
        let proj_self = self.project();
        futures::ready!(proj_self.inner.poll_read(cx, buf))?;
        let read_bytes = initial_remaining - buf.remaining() as u64;
        *proj_self.remaining = proj_self.remaining.saturating_sub(read_bytes);
        if *proj_self.remaining == 0 {
            // The underlying reader has provided more data than we expected.
            io_async_bail!(UnexpectedEof, "Input longer than expected.");
        }

        std::task::Poll::Ready(Ok(()))
    }
}

pub struct Package {
    target_path: Option<PathBuf>,
    metadata: Dirty<Metadata>,
    compressed_data: Dirty<Option<LazyBlock>>,
    raw_data: Dirty<Option<LazyBlock>>,
}

impl Package {
    #[must_use]
    pub fn new(id: ResourceId) -> Self {
        Package {
            target_path: None,
            metadata: Dirty::new_fresh(Metadata::new_with_id(id)),
            compressed_data: Dirty::new_fresh(None),
            raw_data: Dirty::new_fresh(None),
        }
    }

    pub async fn load_from_path<'a, P>(path: P) -> std::io::Result<Self>
    where
        P: Into<Cow<'a, Path>>,
    {
        let path = path.into().into_owned();
        // Some sanity checks. We assume that the files will not be modified
        // concurrently while we are loading them.
        if !path.exists() {
            io_bail!(
                NotFound,
                "Resource package path does not exist: {}",
                path.display()
            );
        }

        let mut meta_file = LengthLimitedAsyncReader {
            inner: tokio::fs::File::options()
                .read(true)
                .open(path.join(META_PATH))
                .await?,
            remaining: 128 * 1024, // 128 KiB
        };

        let mut data = Vec::new();
        meta_file.read_buf(&mut data).await?;
        let metadata: Metadata = serde_json::from_slice(&data)
            .map_err(io_err_map!(InvalidData, "Failed to parse metadata JSON"))?;

        let compressed_path = path.join(COMPRESSED_BIN_PATH);

        let compressed_data = if compressed_path.exists() {
            let block_source = BlockSource::from_path(compressed_path)
                .map_err(io_err_map!(Other, "Failed to create block source"))?;
            Some(LazyBlock::from_block_source(block_source))
        } else {
            None
        };

        let raw_path = path.join(RAW_BIN_PATH);
        let raw_data = if raw_path.exists() {
            let block_source = BlockSource::from_path(raw_path)
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
            target_path: Some(path),
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

    pub fn set_raw_data(&mut self, data: LazyBlock) -> std::io::Result<()> {
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
        // To be maximally safe, the ideal way to save would be to use a multiphase
        // save process, where we write to temporary files, use the meta.json
        // file as an atomic reference, and then push the files into place.
        // This scheme could leave the package in a state where external tools
        // would not be able to find files in it in predictable ways, but
        // could be recovered from.
        //
        // For the time being, we will just overwrite the files directly.

        let Some(path) = &self.target_path else {
            io_bail!(
                InvalidInput,
                "Cannot save a package that was not loaded from a path or saved to a path."
            );
        };

        self.metadata.try_persist(|metadata| {
            let meta_path = path.join(META_PATH);
            let meta_data = serde_json::to_vec(metadata)
                .map_err(io_err_map!(Other, "Failed to serialize metadata to JSON"))?;
            std::fs::write(meta_path, meta_data)
                .map_err(io_err_map!(Other, "Failed to write metadata file"))
        })?;

        self.compressed_data.try_persist(|compressed_data| {
            let compressed_path = path.join(COMPRESSED_BIN_PATH);
            if let Some(compressed_data) = compressed_data {
                let compressed_block = compressed_data
                    .open()
                    .map_err(io_err_map!(Other, "Failed to open compressed data"))?;
                std::fs::write(&compressed_path, compressed_block)
                    .map_err(io_err_map!(Other, "Failed to write compressed data file"))
            } else if compressed_path.exists() {
                std::fs::remove_file(compressed_path)
                    .map_err(io_err_map!(Other, "Failed to remove compressed data file"))
            } else {
                Ok(())
            }
        })?;

        self.raw_data.try_persist(|raw_data| {
            let raw_path = path.join(RAW_BIN_PATH);
            if let Some(raw_data) = raw_data {
                let raw_block = raw_data
                    .open()
                    .map_err(io_err_map!(Other, "Failed to open raw data"))?;
                std::fs::write(&raw_path, raw_block)
                    .map_err(io_err_map!(Other, "Failed to write raw data file"))
            } else if raw_path.exists() {
                std::fs::remove_file(raw_path)
                    .map_err(io_err_map!(Other, "Failed to remove raw data file"))
            } else {
                Ok(())
            }
        })?;

        Ok(())
    }

    /// Saves the package to a new path.
    ///
    /// This will update the stored path of the package to the new path, ensuring
    /// all files are saved there. If this was previously loaded from a path,
    /// the previous files will not be modified, but the old path will be forgotten.
    pub fn save_to(&mut self, path: PathBuf) -> std::io::Result<()> {
        std::fs::create_dir_all(&path)
            .map_err(io_err_map!(Other, "Failed to create package directory"))?;
        let old_target_path = self.target_path.replace(path);
        match self.save() {
            Ok(()) => Ok(()),
            Err(e) => {
                self.target_path = old_target_path;
                Err(e)
            }
        }
    }
}
