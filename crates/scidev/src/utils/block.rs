//! Types that are used to work with ranges of bytes data.

mod block_source;
mod lazy_block;
mod mem_block;
mod output_block;
mod temp_store;

pub use block_source::{
    BlockSource, Error as BlockSourceError, FromBlockSource, FromBlockSourceError,
};
pub use lazy_block::{Error as LazyBlockError, LazyBlock};
pub use mem_block::{FromReaderError as MemBlockFromReaderError, MemBlock};
pub use output_block::OutputBlock;
pub use temp_store::TempStore;

pub mod block2;
