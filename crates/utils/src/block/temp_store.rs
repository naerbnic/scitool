use std::path::Path;

use crate::{block::output_block::OutputBlock, buffer::Buffer};

use super::BlockSource;

pub struct TempStore {
    temp_dir: tempfile::TempDir,
}

impl TempStore {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            temp_dir: tempfile::TempDir::new()?,
        })
    }

    pub fn with_base(base: &Path) -> anyhow::Result<Self> {
        Ok(Self {
            temp_dir: tempfile::TempDir::new_in(base)?,
        })
    }

    pub fn store<B>(&mut self, buffer: B) -> anyhow::Result<BlockSource>
    where
        B: Buffer + Send + Sync + 'static,
    {
        let mut file = tempfile::NamedTempFile::new_in(self.temp_dir.path())?;
        OutputBlock::from_buffer(buffer).write_to(&mut file)?;
        Ok(BlockSource::from_reader(file))
    }
}

#[cfg(test)]
mod tests {
    use bytes::BufMut;

    use super::*;
    use crate::block::MemBlock;

    #[test]
    fn test_temp_store() -> anyhow::Result<()> {
        let mut store = TempStore::new()?;
        let buffer = MemBlock::from_vec(vec![1, 2, 3, 4]);
        let block_source = store.store(buffer)?;
        assert_eq!(block_source.size(), 4);
        let mut read_data = Vec::new();
        read_data.put(block_source.lock().unwrap());
        assert_eq!(read_data, vec![1, 2, 3, 4]);
        Ok(())
    }
}
