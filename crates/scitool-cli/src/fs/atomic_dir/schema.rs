use serde::{Deserialize, Serialize};

use crate::fs::paths::{RelPath, RelPathBuf};

pub(super) const CURR_COMMIT_VERSION: u32 = 1;

/// An entry that indicates that a given file is located at either a temporary
/// path or its final destination.
///
/// This implies that if `temp_path` exists, it should be moved to `dest_path`
/// during a commit operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct OverwriteEntry {
    /// The destination path where the file should be moved to during
    /// a commit operation.
    ///
    /// The equivalent temp file will be at this same path under the temporary
    /// directory. If it does not exist there, it must exist here.
    dest_path: RelPathBuf,
}

impl OverwriteEntry {
    pub(super) fn new(dest_path: RelPathBuf) -> OverwriteEntry {
        OverwriteEntry { dest_path }
    }

    pub(super) fn dest_path(&self) -> &RelPath {
        &self.dest_path
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct DeleteEntry {
    /// The path to delete.
    path: RelPathBuf,
}

impl DeleteEntry {
    pub(super) fn new(path: RelPathBuf) -> DeleteEntry {
        DeleteEntry { path }
    }

    pub(super) fn path(&self) -> &RelPath {
        &self.path
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub(super) enum CommitEntry {
    Overwrite(OverwriteEntry),
    Delete(DeleteEntry),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct CommitSchema {
    version: u32,
    /// The temporary directory used for this commit. It should be immediately
    /// under the dir root directory.
    temp_dir: RelPathBuf,
    /// The list of entries to commit.
    entries: Vec<CommitEntry>,
}

impl CommitSchema {
    pub(super) fn new(temp_dir: RelPathBuf, entries: Vec<CommitEntry>) -> CommitSchema {
        CommitSchema {
            version: CURR_COMMIT_VERSION,
            temp_dir,
            entries,
        }
    }

    pub(super) fn version(&self) -> u32 {
        self.version
    }

    pub(super) fn temp_dir(&self) -> &RelPath {
        &self.temp_dir
    }

    pub(super) fn entries(&self) -> &[CommitEntry] {
        &self.entries
    }

    pub(super) fn take_entries(&mut self) -> Vec<CommitEntry> {
        std::mem::take(&mut self.entries)
    }
}
