//! Types that are used to work with ranges of bytes data.

mod core;
mod mem_block;
mod temp_store;

pub use core::{Block, FromBlock, RefFactory};
pub use mem_block::{FromReaderError as MemBlockFromReaderError, MemBlock};
pub use temp_store::TempStore;
