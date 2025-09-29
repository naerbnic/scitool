use std::ffi::{OsStr, OsString};

use crate::fs::paths::{RelPath, RelPathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FileType {
    is_dir: bool,
}

impl FileType {
    pub(super) fn new_of_dir() -> Self {
        FileType { is_dir: true }
    }

    pub(super) fn new_of_file() -> Self {
        FileType { is_dir: false }
    }

    #[must_use]
    pub fn is_dir(&self) -> bool {
        self.is_dir
    }

    #[must_use]
    pub fn is_file(&self) -> bool {
        !self.is_dir
    }
}

pub struct DirEntry {
    /// The path of the entry, relative to the root of the atomic directory.
    root_path: RelPathBuf,
    file_name: OsString,
    file_type: FileType,
}

impl DirEntry {
    pub(super) fn new(root_path: RelPathBuf, file_name: OsString, file_type: FileType) -> Self {
        DirEntry {
            root_path,
            file_name,
            file_type,
        }
    }
    #[must_use]
    pub fn path(&self) -> RelPathBuf {
        self.root_path
            .join_rel(RelPath::new_checked(&self.file_name).unwrap())
    }

    #[must_use]
    pub fn file_name(&self) -> &OsStr {
        &self.file_name
    }

    #[must_use]
    pub fn file_type(&self) -> FileType {
        self.file_type
    }
}

pub struct Metadata {
    file_type: FileType,
    len: u64,
}

impl Metadata {
    pub(super) fn new(file_type: FileType, len: u64) -> Self {
        Metadata { file_type, len }
    }

    #[must_use]
    pub fn file_type(&self) -> FileType {
        self.file_type
    }

    #[expect(
        clippy::len_without_is_empty,
        reason = "Not being used to represent containers"
    )]
    #[must_use]
    pub fn len(&self) -> u64 {
        self.len
    }
}
