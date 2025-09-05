use std::sync::Arc;

use crate::utils::block::block_source;

use super::{BlockSource, MemBlock};

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Conversion(#[from] std::num::TryFromIntError),
}

impl From<block_source::Error> for Error {
    fn from(value: block_source::Error) -> Self {
        match value {
            block_source::Error::Io(io_err) => Self::Io(io_err),
            block_source::Error::Conversion(conv_err) => Self::Conversion(conv_err),
        }
    }
}

trait LazyBlockImpl: Send + Sync {
    fn open(&self) -> Result<MemBlock, Error>;
    fn size(&self) -> Option<u64>;
}

struct RangeLazyBlockImpl {
    source: BlockSource,
}

impl LazyBlockImpl for RangeLazyBlockImpl {
    fn open(&self) -> Result<MemBlock, Error> {
        Ok(self.source.open()?)
    }

    fn size(&self) -> Option<u64> {
        Some(self.source.size())
    }
}

struct FactoryLazyBlockImpl<F>(F);

impl<F> LazyBlockImpl for FactoryLazyBlockImpl<F>
where
    F: Fn() -> Result<MemBlock, Error> + Send + Sync,
{
    fn open(&self) -> Result<MemBlock, Error> {
        (self.0)()
    }

    fn size(&self) -> Option<u64> {
        None
    }
}

struct MapLazyBlockImpl<F> {
    base_impl: Arc<dyn LazyBlockImpl>,
    map_fn: F,
}

impl<F> LazyBlockImpl for MapLazyBlockImpl<F>
where
    F: Fn(MemBlock) -> Result<MemBlock, Error> + Send + Sync,
{
    fn open(&self) -> Result<MemBlock, Error> {
        let base_block = self.base_impl.open()?;
        (self.map_fn)(base_block)
    }

    fn size(&self) -> Option<u64> {
        None
    }
}

struct MemLazyBlockImpl {
    block: MemBlock,
}

impl LazyBlockImpl for MemLazyBlockImpl {
    fn open(&self) -> Result<MemBlock, Error> {
        Ok(self.block.clone())
    }

    fn size(&self) -> Option<u64> {
        Some(self.block.size() as u64)
    }
}

/// A block that is lazily loaded on demand.
///
/// This can be cheaply cloned, but cannot be split into smaller ranges.
#[derive(Clone)]
pub struct LazyBlock {
    source: Arc<dyn LazyBlockImpl>,
}

impl LazyBlock {
    /// Creates a lazy block that is loaded from a factory on demand.
    pub fn from_factory<F>(factory: F) -> Self
    where
        F: Fn() -> Result<MemBlock, Error> + Send + Sync + 'static,
    {
        Self {
            source: Arc::new(FactoryLazyBlockImpl(factory)),
        }
    }

    #[must_use]
    pub fn from_block_source(source: BlockSource) -> Self {
        Self {
            source: Arc::new(RangeLazyBlockImpl { source }),
        }
    }

    #[must_use]
    pub fn from_mem_block(block: MemBlock) -> Self {
        Self {
            source: Arc::new(MemLazyBlockImpl { block }),
        }
    }

    /// Opens a block from the lazy block source. Returns an error if the block
    /// cannot be loaded.
    pub fn open(&self) -> Result<MemBlock, Error> {
        self.source.open()
    }

    /// Creates a new `LazyBlock` that transforms the result of the current block
    /// with the given function when opened.
    #[must_use]
    pub fn map<F>(self, map_fn: F) -> Self
    where
        F: Fn(MemBlock) -> Result<MemBlock, Error> + Send + Sync + 'static,
    {
        Self {
            source: Arc::new(MapLazyBlockImpl {
                base_impl: self.source,
                map_fn,
            }),
        }
    }

    /// Creates a new lazy block that checks properties about the resulting
    /// block.
    #[must_use]
    pub fn with_check<F>(&self, check_fn: F) -> Self
    where
        F: Fn(&MemBlock) -> Result<(), Error> + Send + Sync + 'static,
    {
        Self {
            source: Arc::new(MapLazyBlockImpl {
                base_impl: self.source.clone(),
                map_fn: move |block| {
                    check_fn(&block)?;
                    Ok(block)
                },
            }),
        }
    }
}

impl std::fmt::Debug for LazyBlock {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("LazyBlock")
            .field("size", &self.source.size())
            .finish()
    }
}
