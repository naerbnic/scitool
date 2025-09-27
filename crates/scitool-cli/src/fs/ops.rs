use std::{
    ffi::{OsStr, OsString},
    fs::TryLockError,
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use futures::Stream;
use tokio::{
    fs::File as TokioFile,
    io::{AsyncRead, AsyncWrite},
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
        self.write || self.append || self.truncate || self.create_new
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

pub trait File: AsyncRead + AsyncWrite + AsyncRead + Unpin + Send {}

impl File for tokio::fs::File {}

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
    fn lock_shared(&self) -> impl Future<Output = io::Result<()>>;
    fn lock_exclusive(&self) -> impl Future<Output = io::Result<()>>;
    fn try_lock_shared(&self) -> impl Future<Output = io::Result<bool>>;
    fn try_lock_exclusive(&self) -> impl Future<Output = io::Result<bool>>;
    fn unlock(&self) -> impl Future<Output = io::Result<()>>;
}

/// A trait for the file system operations needed by `AtomicDir`.
pub trait FileSystemOperations {
    type File: File;
    type FileReader: AsyncRead + Unpin + Send;
    type FileWriter: AsyncWrite + Unpin + Send;
    type FileLock: LockFile + Send;

    /// Checks to see the kind of the path, or if the path does not exist.
    fn get_path_kind(&self, path: &Path) -> impl Future<Output = io::Result<Option<PathKind>>>;

    /// Attempts a rename of the file at `src` to `dst` atomically, overwriting it if it exists.
    /// If the source file does not exist, it returns `RenameFileResult::SourceMissing`
    fn rename_file_atomic(&self, src: &Path, dst: &Path) -> impl Future<Output = io::Result<()>>;

    /// Attempts to create a hard link of the file at `src` to `dst` atomically. If the destination
    /// file already exists, it is not modified and an `AlreadyExists` error is returned.
    ///
    /// Once a hard link is created, the file can be accessed from either path. Deleting one path does not
    /// delete the other path, and the file's data is only removed from disk when all hard links to it are deleted.
    fn link_file_atomic(&self, src: &Path, dst: &Path) -> impl Future<Output = io::Result<()>>;
    fn remove_file(&self, path: &Path) -> impl Future<Output = io::Result<()>>;
    fn remove_dir(&self, path: &Path) -> impl Future<Output = io::Result<()>>;
    fn remove_dir_all(&self, path: &Path) -> impl Future<Output = io::Result<()>>;
    fn list_dir(
        &self,
        path: &Path,
    ) -> impl Future<Output = io::Result<impl Stream<Item = io::Result<DirEntry>> + Unpin>>;
    /// Creates a directory at a location. The parent directories must already exist, and the
    /// directory must not already exist.
    ///
    /// Returns an `AlreadyExists` error if the directory already exists.
    fn create_dir(&self, path: &Path) -> impl Future<Output = io::Result<()>>;
    fn create_dir_all(&self, path: &Path) -> impl Future<Output = io::Result<()>>;
    fn read_file<F, Fut, R>(&self, path: &Path, body: F) -> impl Future<Output = io::Result<R>>
    where
        F: FnOnce(Self::FileReader) -> Fut + Send,
        Fut: Future<Output = io::Result<R>>;
    fn write_to_file<F, Fut, R>(
        &self,
        write_mode: WriteMode,
        path: &Path,
        body: F,
    ) -> impl Future<Output = io::Result<R>>
    where
        F: FnOnce(Self::FileWriter) -> Fut,
        Fut: Future<Output = io::Result<R>>;

    fn open_file(
        &self,
        path: &Path,
        options: &OpenOptionsFlags,
    ) -> impl Future<Output = io::Result<Self::File>>;

    fn open_lock_file(&self, path: &Path) -> impl Future<Output = io::Result<Self::FileLock>>;
}

pub struct TokioFileLock {
    file: Arc<std::fs::File>,
}

impl LockFile for TokioFileLock {
    async fn lock_shared(&self) -> io::Result<()> {
        tokio::task::spawn_blocking({
            let file = self.file.clone();
            move || file.lock_shared()
        })
        .await?
    }

    async fn lock_exclusive(&self) -> io::Result<()> {
        tokio::task::spawn_blocking({
            let file = self.file.clone();
            move || file.lock()
        })
        .await?
    }

    async fn try_lock_shared(&self) -> io::Result<bool> {
        match tokio::task::spawn_blocking({
            let file = self.file.clone();
            move || file.try_lock_shared()
        })
        .await?
        {
            Ok(()) => Ok(true),
            Err(TryLockError::Error(err)) => Err(err),
            Err(TryLockError::WouldBlock) => Ok(false),
        }
    }

    async fn try_lock_exclusive(&self) -> io::Result<bool> {
        match tokio::task::spawn_blocking({
            let file = self.file.clone();
            move || file.try_lock()
        })
        .await?
        {
            Ok(()) => Ok(true),
            Err(TryLockError::Error(err)) => Err(err),
            Err(TryLockError::WouldBlock) => Ok(false),
        }
    }

    async fn unlock(&self) -> io::Result<()> {
        tokio::task::spawn_blocking({
            let file = self.file.clone();
            move || file.unlock()
        })
        .await?
    }
}

pub struct TokioFileSystemOperations;

impl FileSystemOperations for TokioFileSystemOperations {
    type File = tokio::fs::File;
    type FileReader = MutBorrowedArc<TokioFile>;
    type FileWriter = TokioFile;
    type FileLock = TokioFileLock;

    async fn get_path_kind(&self, path: &Path) -> io::Result<Option<PathKind>> {
        match tokio::fs::metadata(&path).await {
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

    async fn rename_file_atomic(&self, src: &Path, dst: &Path) -> io::Result<()> {
        tokio::fs::rename(src, dst).await
    }

    async fn link_file_atomic(&self, src: &Path, dst: &Path) -> io::Result<()> {
        tokio::fs::hard_link(src, dst).await
    }

    async fn remove_file(&self, path: &Path) -> io::Result<()> {
        tokio::fs::remove_file(path).await
    }

    async fn remove_dir(&self, path: &Path) -> io::Result<()> {
        tokio::fs::remove_dir(path).await
    }

    async fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        tokio::fs::remove_dir_all(path).await
    }

    async fn list_dir(
        &self,
        path: &Path,
    ) -> io::Result<impl Stream<Item = io::Result<DirEntry>> + Unpin> {
        let mut entries = tokio::fs::read_dir(path).await?;
        Ok(Box::pin(async_stream::try_stream! {
            while let Some(entry) = entries.next_entry().await? {
                let file_type = entry.file_type().await?;
                let path_kind = if file_type.is_file() {
                    PathKind::File
                } else if file_type.is_dir() {
                    PathKind::Directory
                } else {
                    PathKind::Other
                };
                yield DirEntry::new(path.to_owned(), entry.file_name(), path_kind);
            }
        }))
    }

    async fn create_dir(&self, path: &Path) -> io::Result<()> {
        tokio::fs::create_dir(path).await
    }

    async fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        tokio::fs::create_dir_all(path).await
    }

    async fn read_file<F, Fut, R>(&self, path: &Path, body: F) -> io::Result<R>
    where
        F: FnOnce(Self::FileReader) -> Fut + Send,
        Fut: Future<Output = io::Result<R>>,
    {
        let path = path.to_owned();
        let (borrowed_file, lent_file) = loan_arc(TokioFile::open(&path).await?);
        let res = body(borrowed_file).await;
        let file = lent_file.take_back();
        file.sync_all().await?;
        res
    }

    async fn write_to_file<F, Fut, R>(
        &self,
        write_mode: WriteMode,
        path: &Path,
        body: F,
    ) -> io::Result<R>
    where
        F: FnOnce(Self::FileWriter) -> Fut,
        Fut: Future<Output = io::Result<R>>,
    {
        let path = path.to_owned();
        let mut options = TokioFile::options();
        match write_mode {
            WriteMode::Overwrite => {
                options.create(true).truncate(true);
            }
            WriteMode::CreateNew => {
                options.create_new(true);
            }
        }
        let file = options.write(true).open(&path).await?;
        body(file).await
    }

    async fn open_file(&self, path: &Path, options: &OpenOptionsFlags) -> io::Result<Self::File> {
        let mut open_options = tokio::fs::OpenOptions::new();
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
        let file = open_options.open(path).await?;
        Ok(file)
    }

    async fn open_lock_file(&self, path: &Path) -> io::Result<Self::FileLock> {
        let file = tokio::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .await?;

        let std_file = file.into_std().await;
        Ok(TokioFileLock {
            file: Arc::new(std_file),
        })
    }
}
