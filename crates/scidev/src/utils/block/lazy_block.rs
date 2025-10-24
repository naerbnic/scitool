use std::io;

use crate::utils::{
    block::{block_source, block2::Block},
    errors::OtherError,
};

use super::MemBlock;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Conversion(#[from] std::num::TryFromIntError),
    #[error(transparent)]
    Other(#[from] OtherError),
}

impl Error {
    pub fn from_other<E>(err: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Other(OtherError::new(err))
    }
}

impl From<block_source::Error> for Error {
    fn from(value: block_source::Error) -> Self {
        match value {
            block_source::Error::Io(io_err) => Self::Io(io_err),
            block_source::Error::Conversion(conv_err) => Self::Conversion(conv_err),
        }
    }
}

impl From<Error> for io::Error {
    fn from(value: Error) -> Self {
        match value {
            Error::Io(error) => error,
            Error::Conversion(try_from_int_error) => io::Error::other(try_from_int_error),
            Error::Other(other_error) => io::Error::other(other_error),
        }
    }
}

/// A block that is lazily loaded on demand.
///
/// This can be cheaply cloned, but cannot be split into smaller ranges.
#[derive(Clone)]
pub struct LazyBlock {
    block: Block,
}

impl LazyBlock {
    pub fn from_error<F>(err: F) -> Self
    where
        F: Fn() -> Error + Clone + Send + Sync + 'static,
    {
        Self {
            block: Block::from_error_fn(move || io::Error::new(io::ErrorKind::Other, err())),
        }
    }

    #[must_use]
    pub fn from_block_source(source: Block) -> Self {
        Self { block: source }
    }

    #[must_use]
    pub fn from_mem_block(block: MemBlock) -> Self {
        Self {
            block: Block::from_mem_block(block),
        }
    }

    /// Opens a block from the lazy block source. Returns an error if the block
    /// cannot be loaded.
    pub fn open(&self) -> Result<MemBlock, Error> {
        self.block.open_mem(..).map_err(Error::Io)
    }

    /// Creates a new `LazyBlock` that transforms the result of the current block
    /// with the given function when opened.
    #[must_use]
    pub fn map<F>(self, map_fn: F) -> Result<Self, Error>
    where
        F: Fn(MemBlock) -> Result<MemBlock, Error> + Send + Sync + 'static,
    {
        // Ok(Self {
        //     block: Builder::new().build_from_mem_block_factory({
        //         let this = self.clone();
        //         move || {
        //             let mem_block = this.open()?;
        //             Ok(map_fn(mem_block)?)
        //         }
        //     })?,
        // })

        let mem_block = self.open()?;
        Ok(Self {
            block: Block::from_mem_block(map_fn(mem_block)?),
        })
    }

    /// Creates a new lazy block that checks properties about the resulting
    /// block.
    #[must_use]
    pub fn with_check<F>(&self, check_fn: F) -> Result<Self, Error>
    where
        F: Fn(&MemBlock) -> Result<(), Error> + Send + Sync + 'static,
    {
        // Ok(Self {
        //     block: Builder::new().build_from_mem_block_factory({
        //         let this = self.clone();
        //         move || {
        //             let mem_block = this.open()?;
        //             check_fn(&mem_block)?;
        //             Ok(mem_block)
        //         }
        //     })?,
        // })
        let mem_block = self.open()?;
        check_fn(&mem_block)?;
        Ok(Self {
            block: Block::from_mem_block(mem_block),
        })
    }
}

impl std::fmt::Debug for LazyBlock {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("LazyBlock")
            .field("size", &self.block.len())
            .finish()
    }
}
