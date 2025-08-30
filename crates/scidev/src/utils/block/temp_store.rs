use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use bytes::Buf;

use crate::utils::{
    buffer::Buffer,
    errors::other::{OtherError, ResultExt},
};

use super::BlockSource;

struct BlockPathHandle {
    path: PathBuf,
    // This is used to keep the temp file alive
    _dir: Arc<tempfile::TempDir>,
}

impl AsRef<Path> for BlockPathHandle {
    fn as_ref(&self) -> &Path {
        self.path.as_ref()
    }
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CreateError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum StoreError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[doc(hidden)]
    #[error(transparent)]
    Other(#[from] OtherError),
}

pub struct TempStore {
    temp_dir: Arc<tempfile::TempDir>,
}

impl TempStore {
    pub fn create() -> Result<Self, CreateError> {
        Ok(Self {
            temp_dir: Arc::new(tempfile::TempDir::new()?),
        })
    }

    pub fn with_base(base: &Path) -> Result<Self, CreateError> {
        Ok(Self {
            temp_dir: Arc::new(tempfile::TempDir::new_in(base)?),
        })
    }

    pub async fn store_bytes<B>(&mut self, buffer: B) -> Result<BlockSource, StoreError>
    where
        B: Buf,
    {
        self.create_temp_block(buffer).await
    }

    pub async fn store<B>(&mut self, buffer: B) -> Result<BlockSource, StoreError>
    where
        B: Buffer + Send + Sync + 'static,
    {
        self.create_temp_block(buffer.lock().with_other_err()?)
            .await
    }

    async fn create_temp_block<B>(&self, buffer: B) -> Result<BlockSource, StoreError>
    where
        B: Buf,
    {
        let (mut file, path) = tempfile::NamedTempFile::new_in(self.temp_dir.path())?
            .keep()
            .with_other_err()?;
        std::io::copy(&mut buffer.reader(), &mut file)?;
        drop(file);
        Ok(BlockSource::from_path(BlockPathHandle {
            path,
            _dir: self.temp_dir.clone(),
        })?)
    }
}

#[cfg(test)]
mod tests {
    use bytes::BufMut;

    use super::*;
    use crate::utils::block::MemBlock;
    #[tokio::test]
    async fn test_temp_store() -> anyhow::Result<()> {
        let mut store = TempStore::create()?;
        let buffer = MemBlock::from_vec(vec![1, 2, 3, 4]);
        let block_source = store.store(buffer).await?;
        assert_eq!(block_source.size(), 4);
        let mut read_data = Vec::new();
        read_data.put(block_source.lock().unwrap());
        assert_eq!(read_data, vec![1, 2, 3, 4]);
        Ok(())
    }
}
