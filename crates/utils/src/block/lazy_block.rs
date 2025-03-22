use std::sync::Arc;

use super::{Block, BlockSource, ReadResult};

trait LazyBlockImpl {
    fn open(&self) -> ReadResult<Block>;
    fn size(&self) -> Option<u64>;
}

struct RangeLazyBlockImpl {
    source: BlockSource,
}

impl LazyBlockImpl for RangeLazyBlockImpl {
    fn open(&self) -> ReadResult<Block> {
        self.source.open()
    }

    fn size(&self) -> Option<u64> {
        Some(self.source.size())
    }
}

struct FactoryLazyBlockImpl<F>(F);

impl<F> LazyBlockImpl for FactoryLazyBlockImpl<F>
where
    F: Fn() -> ReadResult<Block>,
{
    fn open(&self) -> ReadResult<Block> {
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
    F: Fn(Block) -> ReadResult<Block>,
{
    fn open(&self) -> ReadResult<Block> {
        let base_block = self.base_impl.open()?;
        (self.map_fn)(base_block)
    }

    fn size(&self) -> Option<u64> {
        None
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
        F: Fn() -> ReadResult<Block> + 'static,
    {
        Self {
            source: Arc::new(FactoryLazyBlockImpl(factory)),
        }
    }

    pub fn from_block_source(source: BlockSource) -> Self {
        Self {
            source: Arc::new(RangeLazyBlockImpl { source }),
        }
    }

    /// Opens a block from the lazy block source. Returns an error if the block
    /// cannot be loaded.
    pub fn open(&self) -> ReadResult<Block> {
        self.source.open()
    }

    /// Creates a new LazyBlock that transforms the result of the current block
    /// with the given function when opened.
    pub fn map<F>(self, map_fn: F) -> Self
    where
        F: Fn(Block) -> ReadResult<Block> + 'static,
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
    pub fn with_check<F>(&self, check_fn: F) -> Self
    where
        F: Fn(&Block) -> ReadResult<()> + 'static,
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
