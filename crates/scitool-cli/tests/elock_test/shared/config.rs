use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct TimeRange {
    pub min: std::time::Duration,
    pub max: std::time::Duration,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct WorkerConfig {
    pub lock_file_path: PathBuf,
    pub hold_ms: TimeRange,
    pub use_shared: bool,
}
