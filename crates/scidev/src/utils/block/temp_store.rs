use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use scidev_macros_internal::other_fn;

use crate::utils::{
    block::Block,
    buffer::{Buffer, BufferCursor},
    errors::{BoxError, OpaqueError},
};

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
    Other(#[from] OpaqueError),
}

impl From<BoxError> for StoreError {
    fn from(err: BoxError) -> Self {
        StoreError::Other(OpaqueError::from_boxed(err))
    }
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

    pub fn store_bytes<B>(&mut self, buffer: B) -> Result<Block, StoreError>
    where
        B: Buffer,
    {
        self.create_temp_block(buffer)
    }

    pub fn store<B>(&mut self, buffer: B) -> Result<Block, StoreError>
    where
        B: Buffer,
    {
        self.create_temp_block(buffer)
    }

    #[other_fn]
    fn create_temp_block<B>(&self, buffer: B) -> Result<Block, StoreError>
    where
        B: Buffer,
    {
        let (mut file, path) = tempfile::NamedTempFile::new_in(self.temp_dir.path())?.keep()?;
        std::io::copy(&mut BufferCursor::new(buffer.into_fallible()), &mut file)?;
        drop(file);
        Ok(Block::from_path(BlockPathHandle {
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
    #[cfg_attr(miri, ignore)]
    #[test]
    fn test_temp_store() -> anyhow::Result<()> {
        let mut store = TempStore::create()?;
        let buffer = MemBlock::from_vec(vec![1, 2, 3, 4]);
        let block_source = store.store(buffer)?;
        assert_eq!(block_source.len(), 4);
        let mut read_data = Vec::new();
        read_data.put(BufferCursor::new(block_source.open_mem(..)?));
        assert_eq!(read_data, vec![1, 2, 3, 4]);
        Ok(())
    }
}
