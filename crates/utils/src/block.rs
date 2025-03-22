//! Types that are used to work with ranges of bytes data.

mod block_reader;
mod block_source;
mod core;
mod error;
mod lazy_block;

pub use block_reader::BlockReader;
pub use block_source::BlockSource;
pub use core::Block;
pub use error::{ReadError, ReadResult};
pub use lazy_block::LazyBlock;
