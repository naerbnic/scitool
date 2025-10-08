use std::io;

use serde::{Deserialize, Serialize};

use crate::fs::err_helpers::io_err;

const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum StateLoadError {
    #[error("Unable to parse directory status: {0}")]
    FormatError(Box<dyn std::error::Error + Send + Sync>),
    #[error("Unsupported schema version: {0}")]
    UnsupportedSchemaVersion(u32),
}

impl From<StateLoadError> for io::Error {
    fn from(err: StateLoadError) -> Self {
        match err {
            StateLoadError::FormatError(e) => {
                io_err!(InvalidData, "Unable to parse directory status: {}", e)
            }
            StateLoadError::UnsupportedSchemaVersion(v) => {
                io_err!(InvalidData, "Unsupported schema version: {}", v)
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Schema {
    #[expect(clippy::struct_field_names, reason = "Intended for schema versioning")]
    schema: u32,
    sequence: u32,
    poisoned: bool,
}

pub(super) enum LoadedDirState {
    Clean(DirState),
    Poisoned,
}

#[derive(Debug, Clone)]
pub(super) struct DirState {
    inner: Schema,
}

impl DirState {
    pub(super) fn new() -> Self {
        Self {
            inner: Schema {
                schema: CURRENT_SCHEMA_VERSION,
                sequence: 1,
                poisoned: false,
            },
        }
    }

    pub(super) fn load(data: &[u8]) -> Result<LoadedDirState, StateLoadError> {
        let inner: Schema =
            serde_json::from_slice(data).map_err(|e| StateLoadError::FormatError(Box::new(e)))?;
        if inner.schema != CURRENT_SCHEMA_VERSION {
            return Err(StateLoadError::UnsupportedSchemaVersion(inner.schema));
        }
        if inner.poisoned {
            return Ok(LoadedDirState::Poisoned);
        }
        Ok(LoadedDirState::Clean(Self { inner }))
    }

    pub(super) fn serialize(&self) -> io::Result<Vec<u8>> {
        serde_json::to_vec(&self.inner)
            .map_err(|e| io_err!(Other, "Failed to serialize directory state: {}", e))
    }

    pub(super) fn to_next(&self) -> Self {
        Self {
            inner: Schema {
                schema: self.inner.schema,
                sequence: self.inner.sequence.wrapping_add(1),
                poisoned: self.inner.poisoned,
            },
        }
    }

    pub(super) fn is_same(&self, other: &DirState) -> bool {
        self.inner.sequence == other.inner.sequence
    }
}
