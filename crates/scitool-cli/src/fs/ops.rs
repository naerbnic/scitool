use std::{
    fs::TryLockError,
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use futures::Stream;
use tokio::{
    fs::File,
    io::{AsyncRead, AsyncWrite},
};

use crate::fs::owned_arc::{MutBorrowedArc, loan_arc};

pub enum PathKind {
    File,
    Directory,
    Other,
    Missing,
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

pub trait LockFile {
    fn lock_shared(&self) -> impl Future<Output = io::Result<()>>;
    fn lock_exclusive(&self) -> impl Future<Output = io::Result<()>>;
    fn try_lock_shared(&self) -> impl Future<Output = io::Result<bool>>;
    fn try_lock_exclusive(&self) -> impl Future<Output = io::Result<bool>>;
    fn unlock(&self) -> impl Future<Output = io::Result<()>>;
}

/// A trait for the file system operations needed by `AtomicDir`.
pub trait FileSystemOperations {
    type FileReader: AsyncRead + Unpin + Send;
    type FileWriter: AsyncWrite + Unpin + Send;
    type FileLock: LockFile + Send;

    /// Checks to see the kind of the path, or if the path does not exist.
    fn get_path_kind(&self, path: &Path) -> impl Future<Output = io::Result<PathKind>>;

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
    ) -> impl Future<Output = io::Result<impl Stream<Item = io::Result<PathBuf>> + Unpin>>;
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
    type FileReader = MutBorrowedArc<File>;
    type FileWriter = File;
    type FileLock = TokioFileLock;

    async fn get_path_kind(&self, path: &Path) -> io::Result<PathKind> {
        match tokio::fs::metadata(&path).await {
            Ok(metadata) => {
                if metadata.is_file() {
                    Ok(PathKind::File)
                } else if metadata.is_dir() {
                    Ok(PathKind::Directory)
                } else {
                    Ok(PathKind::Other)
                }
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(PathKind::Missing),
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
    ) -> io::Result<impl Stream<Item = io::Result<PathBuf>> + Unpin> {
        let mut entries = tokio::fs::read_dir(path).await?;
        Ok(futures::stream::poll_fn(move |cx| {
            let poll_result = futures::ready!(entries.poll_next_entry(cx));
            let entry = match poll_result {
                Ok(entry) => entry,
                Err(err) => return std::task::Poll::Ready(Some(Err(err))),
            };
            if let Some(entry) = entry {
                std::task::Poll::Ready(Some(Ok(entry.path())))
            } else {
                std::task::Poll::Ready(None)
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
        let (borrowed_file, lent_file) = loan_arc(File::open(&path).await?);
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
        let mut options = File::options();
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
