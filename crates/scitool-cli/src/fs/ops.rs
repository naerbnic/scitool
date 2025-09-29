use std::{
    ffi::{OsStr, OsString},
    fs::{Metadata, TryLockError},
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::fs::owned_arc::{MutBorrowedArc, loan_arc};

#[expect(
    clippy::struct_excessive_bools,
    reason = "Flags struct to map to std::fs::OpenOptions"
)]
#[derive(Default, Clone)]
pub struct OpenOptionsFlags {
    read: bool,
    write: bool,
    append: bool,
    truncate: bool,
    create: bool,
    create_new: bool,
}

impl OpenOptionsFlags {
    #[must_use]
    pub fn read(&self) -> bool {
        self.read
    }
    #[must_use]
    pub fn write(&self) -> bool {
        self.write
    }
    #[must_use]
    pub fn append(&self) -> bool {
        self.append
    }
    #[must_use]
    pub fn truncate(&self) -> bool {
        self.truncate
    }
    #[must_use]
    pub fn create(&self) -> bool {
        self.create
    }
    #[must_use]
    pub fn create_new(&self) -> bool {
        self.create_new
    }

    #[must_use]
    pub fn can_change_file(&self) -> bool {
        // We need a temporary file if we are going to modify the file.
        //
        // This is used to determine if we need to provide a temporary path for writing.
        self.write || self.append || self.truncate || self.create_new || self.create
    }

    #[must_use]
    pub fn can_create_file(&self) -> bool {
        // We can create the file if we are allowed to create it, or if we are truncating it.
        self.create || self.create_new
    }

    #[must_use]
    pub fn uses_original_data(&self) -> bool {
        // The options are read-only if an existing file is opened for reading only, or
        // if there is no file, no file will be created.
        //
        // This is used to detect if we need to copy the file to a temporary location
        // before writing to it.

        // If we are truncating, the original data will be erased, so we do not use it.
        !self.truncate || !self.create_new
    }

    pub fn set_read(&mut self, read: bool) {
        self.read = read;
    }

    pub fn set_write(&mut self, write: bool) {
        self.write = write;
    }

    pub fn set_append(&mut self, append: bool) {
        self.append = append;
    }

    pub fn set_truncate(&mut self, truncate: bool) {
        self.truncate = truncate;
    }

    pub fn set_create(&mut self, create: bool) {
        self.create = create;
    }

    pub fn set_create_new(&mut self, create_new: bool) {
        self.create_new = create_new;
    }
}

pub trait File: io::Read + io::Write {}

impl File for std::fs::File {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathKind {
    File,
    Directory,
    Other,
}

#[derive(Debug)]
pub enum WriteMode {
    /// Overwrite the file if it exists, or create it if it does not.
    Overwrite,
    /// Create the file, but fail if it already exists.
    CreateNew,
}

pub enum LockStatus {
    None,
    Shared,
    Exclusive,
}

pub struct DirEntry {
    root_path: PathBuf,
    file_name: OsString,
    file_type: PathKind,
}

impl DirEntry {
    #[must_use]
    pub fn new(root_path: PathBuf, file_name: OsString, file_type: PathKind) -> Self {
        Self {
            root_path,
            file_name,
            file_type,
        }
    }

    #[must_use]
    pub fn path(&self) -> PathBuf {
        self.root_path.join(&self.file_name)
    }

    #[must_use]
    pub fn file_name(&self) -> &OsStr {
        &self.file_name
    }

    #[must_use]
    pub fn file_type(&self) -> PathKind {
        self.file_type
    }
}

pub trait LockFile {
    fn lock_shared(&self) -> io::Result<()>;
    fn lock_exclusive(&self) -> io::Result<()>;
    fn try_lock_shared(&self) -> io::Result<bool>;
    fn try_lock_exclusive(&self) -> io::Result<bool>;
    fn unlock(&self) -> io::Result<()>;
}

/// A trait for the file system operations needed by `AtomicDir`.
pub trait FileSystemOperations: Send {
    type File: File;
    type FileReader: io::Read;
    type FileWriter: io::Write;
    type FileLock: LockFile;

    /// Checks to see the kind of the path, or if the path does not exist.
    fn get_path_kind(&self, path: &Path) -> io::Result<Option<PathKind>>;

    /// Attempts a rename of the file at `src` to `dst` atomically, overwriting it if it exists.
    /// If the source file does not exist, it returns `RenameFileResult::SourceMissing`
    fn rename_file_atomic(&self, src: &Path, dst: &Path) -> io::Result<()>;

    /// Attempts to create a hard link of the file at `src` to `dst` atomically. If the destination
    /// file already exists, it is not modified and an `AlreadyExists` error is returned.
    ///
    /// Once a hard link is created, the file can be accessed from either path. Deleting one path does not
    /// delete the other path, and the file's data is only removed from disk when all hard links to it are deleted.
    fn link_file_atomic(&self, src: &Path, dst: &Path) -> io::Result<()>;
    fn remove_file(&self, path: &Path) -> io::Result<()>;
    fn remove_dir(&self, path: &Path) -> io::Result<()>;
    fn remove_dir_all(&self, path: &Path) -> io::Result<()>;
    fn list_dir(&self, path: &Path) -> io::Result<impl Iterator<Item = io::Result<DirEntry>>>;
    /// Creates a directory at a location. The parent directories must already exist, and the
    /// directory must not already exist.
    ///
    /// Returns an `AlreadyExists` error if the directory already exists.
    fn create_dir(&self, path: &Path) -> io::Result<()>;
    fn create_dir_all(&self, path: &Path) -> io::Result<()>;
    fn read_file<F, R>(&self, path: &Path, body: F) -> io::Result<R>
    where
        F: FnOnce(Self::FileReader) -> io::Result<R>;
    fn write_to_file<F, R>(&self, write_mode: WriteMode, path: &Path, body: F) -> io::Result<R>
    where
        F: FnOnce(Self::FileWriter) -> io::Result<R>;

    fn open_file(&self, path: &Path, options: &OpenOptionsFlags) -> io::Result<Self::File>;

    fn open_lock_file(&self, path: &Path) -> io::Result<Self::FileLock>;

    fn metadata(&self, path: &Path) -> io::Result<Metadata>;
}

pub struct TokioFileLock {
    file: Arc<std::fs::File>,
}

impl LockFile for TokioFileLock {
    fn lock_shared(&self) -> io::Result<()> {
        self.file.lock_shared()
    }

    fn lock_exclusive(&self) -> io::Result<()> {
        self.file.clone().lock()
    }

    fn try_lock_shared(&self) -> io::Result<bool> {
        match self.file.try_lock_shared() {
            Ok(()) => Ok(true),
            Err(TryLockError::Error(err)) => Err(err),
            Err(TryLockError::WouldBlock) => Ok(false),
        }
    }

    fn try_lock_exclusive(&self) -> io::Result<bool> {
        match self.file.try_lock() {
            Ok(()) => Ok(true),
            Err(TryLockError::Error(err)) => Err(err),
            Err(TryLockError::WouldBlock) => Ok(false),
        }
    }

    fn unlock(&self) -> io::Result<()> {
        self.file.unlock()
    }
}

pub struct StdFileSystemOperations;

impl FileSystemOperations for StdFileSystemOperations {
    type File = std::fs::File;
    type FileReader = std::fs::File;
    type FileWriter = MutBorrowedArc<std::fs::File>;
    type FileLock = TokioFileLock;

    fn get_path_kind(&self, path: &Path) -> io::Result<Option<PathKind>> {
        match std::fs::metadata(path) {
            Ok(metadata) => {
                if metadata.is_file() {
                    Ok(Some(PathKind::File))
                } else if metadata.is_dir() {
                    Ok(Some(PathKind::Directory))
                } else {
                    Ok(Some(PathKind::Other))
                }
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err),
        }
    }

    fn rename_file_atomic(&self, src: &Path, dst: &Path) -> io::Result<()> {
        std::fs::rename(src, dst)
    }

    fn link_file_atomic(&self, src: &Path, dst: &Path) -> io::Result<()> {
        std::fs::hard_link(src, dst)
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        std::fs::remove_file(path)
    }

    fn remove_dir(&self, path: &Path) -> io::Result<()> {
        std::fs::remove_dir(path)
    }

    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        std::fs::remove_dir_all(path)
    }

    fn list_dir(&self, path: &Path) -> io::Result<impl Iterator<Item = io::Result<DirEntry>>> {
        let entries = std::fs::read_dir(path)?;
        Ok(entries.map(|entry_res| {
            let entry = entry_res?;
            let file_type = entry.file_type()?;
            let path_kind = if file_type.is_file() {
                PathKind::File
            } else if file_type.is_dir() {
                PathKind::Directory
            } else {
                PathKind::Other
            };
            Ok(DirEntry::new(path.to_owned(), entry.file_name(), path_kind))
        }))
    }

    fn create_dir(&self, path: &Path) -> io::Result<()> {
        std::fs::create_dir(path)
    }

    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        std::fs::create_dir_all(path)
    }

    fn read_file<F, R>(&self, path: &Path, body: F) -> io::Result<R>
    where
        F: FnOnce(Self::FileReader) -> io::Result<R>,
    {
        let path = path.to_owned();
        body(std::fs::File::open(&path)?)
    }

    fn write_to_file<F, R>(&self, write_mode: WriteMode, path: &Path, body: F) -> io::Result<R>
    where
        F: FnOnce(Self::FileWriter) -> io::Result<R>,
    {
        let path = path.to_owned();
        let mut options = std::fs::File::options();
        match write_mode {
            WriteMode::Overwrite => {
                options.create(true).truncate(true);
            }
            WriteMode::CreateNew => {
                options.create_new(true);
            }
        }
        let file = options.write(true).open(&path)?;
        let (borrowed_file, lent_file) = loan_arc(file);
        let result = body(borrowed_file)?;
        let file = lent_file.take_back();
        file.sync_all()?;
        Ok(result)
    }

    fn open_file(&self, path: &Path, options: &OpenOptionsFlags) -> io::Result<Self::File> {
        let mut open_options = std::fs::File::options();
        if options.read {
            open_options.read(true);
        }
        if options.write {
            open_options.write(true);
        }
        if options.append {
            open_options.append(true);
        }
        if options.truncate {
            open_options.truncate(true);
        }
        if options.create {
            open_options.create(true);
        }
        if options.create_new {
            open_options.create_new(true);
        }
        let file = open_options.open(path)?;
        Ok(file)
    }

    fn open_lock_file(&self, path: &Path) -> io::Result<Self::FileLock> {
        let file = std::fs::File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;

        Ok(TokioFileLock {
            file: Arc::new(file),
        })
    }

    fn metadata(&self, path: &Path) -> io::Result<Metadata> {
        std::fs::metadata(path)
    }
}
