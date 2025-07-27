//! Types that are used to work with ranges of bytes data.

mod block_reader;
mod block_source;
pub mod cache_store;
mod error;
mod lazy_block;
mod mem_block;
pub mod output_block;
pub mod temp_store;

pub use block_reader::BlockReader;
pub use block_source::BlockSource;
pub use error::{ReadError, ReadResult};
pub use lazy_block::LazyBlock;
pub use mem_block::MemBlock;
