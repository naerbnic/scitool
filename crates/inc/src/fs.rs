mod std_fs;

use asynx::io as async_io;
use scidev_path::{relpath::RelPath, segment::Segment};
use std::io;

use crate::promise::{PromiseStream, SharedPromise};

/// What action to take if a file exists when opening it.
pub enum ExistingAction {
    /// Preserve file contents.
    Preserve,

    /// Truncate file contents.
    Truncate,

    /// Fail the operation if file exists.
    Fail,
}

/// What action to take if a file does not exist when opening it.
pub enum MissingAction {
    /// Create a new file.
    Create,

    /// Fail the operation if the file does not exist.
    Fail,
}

enum AccessMode {
    /// Read only file. File must already exist.
    ReadOnly,

    /// A mode where the underlying file will change.
    Mutable(MutableMode),
}

/// What operations can be taken on an opened file.
pub enum MutationMode {
    /// Open the file for writing only.
    WriteOnly,

    /// Open the file for reading and writing using a shared cursor position.
    ReadWrite,

    /// Open the file for appending. Cannot read from it, or meaningfully
    /// seek on it.
    Append,
}

struct MutableMode {
    /// How the file can be accessed once opened.
    mode: MutationMode,

    /// What to do with an existing file.
    existing: ExistingAction,

    /// What to do with a new file.
    missing: MissingAction,
}

pub struct OpenOptions {
    access_mode: AccessMode,
}

impl OpenOptions {
    #[must_use]
    pub fn new_read_only() -> Self {
        Self {
            access_mode: AccessMode::ReadOnly,
        }
    }

    #[must_use]
    pub fn new_mutable(
        mode: MutationMode,
        existing: ExistingAction,
        missing: MissingAction,
    ) -> Self {
        Self {
            access_mode: AccessMode::Mutable(MutableMode {
                mode,
                existing,
                missing,
            }),
        }
    }

    fn to_std_options(&self) -> io::Result<std::fs::OpenOptions> {
        let mut opts = std::fs::OpenOptions::new();
        match &self.access_mode {
            AccessMode::ReadOnly => {
                opts.read(true);
            }
            AccessMode::Mutable(mode) => {
                match mode.mode {
                    MutationMode::WriteOnly => {
                        opts.write(true);
                    }
                    MutationMode::ReadWrite => {
                        opts.read(true).write(true);
                    }
                    MutationMode::Append => {
                        opts.append(true);
                    }
                }
                match (&mode.existing, &mode.missing) {
                    (ExistingAction::Preserve, MissingAction::Fail) => {
                        opts.write(true);
                    }
                    (ExistingAction::Truncate, MissingAction::Fail) => {
                        opts.truncate(true);
                    }
                    (ExistingAction::Preserve, MissingAction::Create) => {
                        opts.write(true).create(true);
                    }
                    (ExistingAction::Truncate, MissingAction::Create) => {
                        opts.truncate(true).create(true);
                    }
                    (ExistingAction::Fail, MissingAction::Create) => {
                        opts.create_new(true);
                    }
                    (ExistingAction::Fail, MissingAction::Fail) => {
                        return Err(io::Error::new(
                            io::ErrorKind::Unsupported,
                            "Must either be able to open an existing or new file.",
                        ));
                    }
                }
            }
        }
        Ok(opts)
    }
}

pub struct Metadata {}

pub struct DirEntry {}

pub trait SyncFile: io::Read + io::Write + io::Seek + Unpin + Send + 'static {}

pub trait AsyncFile:
    async_io::AsyncRead + async_io::AsyncWrite + async_io::AsyncSeek + Unpin + Send + 'static
{
}

pub trait VfsProvider {
    /// Represents an open file. Note that even though the file is open, you
    /// must call one of the `file_io` methods to get an object that can read and
    /// write data from a file.
    type FileHandle: Send + Sync + 'static;
    type DirHandle: Send + Sync + 'static;

    type SyncFile: SyncFile;
    type AsyncFile: AsyncFile;

    fn open_file_handle(
        &self,
        path: RelPath,
        open_options: OpenOptions,
    ) -> SharedPromise<io::Result<Self::FileHandle>>;
    fn open_dir_handle(&self, path: RelPath) -> SharedPromise<io::Result<Self::DirHandle>>;

    fn get_file_metadata(&self, file: &Self::FileHandle) -> SharedPromise<io::Result<Metadata>>;
    fn set_file_length(&self, file: &Self::FileHandle, size: u64) -> SharedPromise<io::Result<()>>;
    fn set_file_times(
        &self,
        file: &Self::FileHandle,
        times: &std::fs::FileTimes,
    ) -> SharedPromise<io::Result<()>>;
    fn sync_file(&self, file: &Self::FileHandle) -> SharedPromise<io::Result<()>>;
    fn delete_file(&self, file: &Self::FileHandle) -> SharedPromise<io::Result<()>>;

    fn open_sync_file_io(
        &self,
        file: Self::FileHandle,
    ) -> SharedPromise<io::Result<Self::SyncFile>>;
    fn open_async_file_io(
        &self,
        file: Self::FileHandle,
    ) -> SharedPromise<io::Result<Self::AsyncFile>>;

    fn read_dir(&self, dir: &Self::DirHandle) -> PromiseStream<io::Result<DirEntry>>;
    fn make_dir(
        &self,
        base: &Self::DirHandle,
        name: &Segment,
    ) -> SharedPromise<io::Result<Self::DirHandle>>;
    fn delete_dir(&self, dir: &Self::DirHandle) -> SharedPromise<io::Result<()>>;
}
