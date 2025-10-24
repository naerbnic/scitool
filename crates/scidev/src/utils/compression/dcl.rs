mod compress;
mod decompress;
mod header;
mod trees;

pub use compress::{compress_dcl, compress_reader};
pub use decompress::{DecompressFactory, DecompressionError, decompress_dcl, decompress_reader};
pub use header::{CompressionMode, DictType};

#[cfg(test)]
mod tests;
