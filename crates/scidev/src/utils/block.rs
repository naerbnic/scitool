//! Types that are used to work with ranges of bytes data.

mod async_read;
mod builder;
mod core;
mod mem_block;
mod temp_store;

pub use builder::{BlockBuilder, BlockBuilderFactory};
pub use core::{Block, FromBlock, FromPathError, FromPathErrorKind, OpenError};
pub use mem_block::{CachedMemBlock, FromReaderError as MemBlockFromReaderError, MemBlock};
pub use temp_store::TempStore;
