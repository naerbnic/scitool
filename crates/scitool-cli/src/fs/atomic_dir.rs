//! Atomic directory implementation.

mod engine;
mod recovery;
mod schema;
mod types;
mod util;

use std::{
    io::{self, SeekFrom},
    path::Path,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use futures::TryStream;
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncSeek, AsyncWrite, AsyncWriteExt as _, ReadBuf};

use crate::fs::{
    atomic_dir::engine::Engine,
    open_tracker::{OpenMarker, OpenTracker},
    ops::{OpenOptionsFlags, TokioFileSystemOperations, WriteMode},
};

pub use self::types::{DirEntry, FileType, Metadata};

const LOCK_PATH: &str = ".DIR_LOCK";
const COMMIT_PATH: &str = ".DIR_COMMIT";

#[pin_project::pin_project]
pub struct AtomicDirFile {
    #[pin]
    file: tokio::fs::File,
    tracker: WrapperTracker,
}

impl AtomicDirFile {
    fn new(file: tokio::fs::File, tracker: WrapperTracker) -> Self {
        Self { file, tracker }
    }

    pub async fn close(self) -> io::Result<()> {
        // First close the file itself, capturing any errors.
        let close_result = self.file.sync_all().await;
        std::mem::drop(self.file);

        // Try to abort the parent AtomicDir if this is the last reference.
        //
        // This will not happen if there is still a top-level AtomicDir object
        // alive, or if there are other files still open.
        //
        // Doing this allows us to propagate any errors that occur during
        // cleanup, which would otherwise be ignored.
        if let Some(parent) = self.tracker.into_inner() {
            parent.abort().await?;
        }

        // Now propagate any errors from closing the file itself.
        close_result?;

        Ok(())
    }
}

impl AsyncRead for AtomicDirFile {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        self.project().file.poll_read(cx, buf)
    }
}

impl AsyncWrite for AtomicDirFile {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.project().file.poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.project().file.poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.project().file.poll_shutdown(cx)
    }
}

impl AsyncSeek for AtomicDirFile {
    fn start_seek(self: Pin<&mut Self>, position: SeekFrom) -> io::Result<()> {
        self.project().file.start_seek(position)
    }

    fn poll_complete(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<u64>> {
        self.project().file.poll_complete(cx)
    }
}

struct WrapperTracker {
    parent: Option<Arc<Engine<TokioFileSystemOperations>>>,
    open_marker: Option<OpenMarker>,
}

impl WrapperTracker {
    fn new(parent: Arc<Engine<TokioFileSystemOperations>>, open_marker: OpenMarker) -> Self {
        Self {
            parent: Some(parent),
            open_marker: Some(open_marker),
        }
    }

    fn into_inner(mut self) -> Option<Engine<TokioFileSystemOperations>> {
        Arc::into_inner(self.parent.take().unwrap())
    }
}

impl Drop for WrapperTracker {
    fn drop(&mut self) {
        std::mem::drop(self.parent.take());
        std::mem::drop(self.open_marker.take());
    }
}

/// A builder for opening files within an `AtomicDir`.
///
/// This provides a fluent interface for specifying how a file should be opened,
/// similar to `std::fs::OpenOptions`. An `OpenOptions` instance can be used to
/// configure how a file is opened and what operations are permitted on the opened file.
pub struct OpenOptions<'a> {
    parent: &'a Inner,
    flags: OpenOptionsFlags,
}

impl OpenOptions<'_> {
    /// Sets the option for read access.
    ///
    /// This option, when true, will allow the file to be read.
    /// Defaults to `false`.
    pub fn read(&mut self, read: bool) -> &mut Self {
        self.flags.set_read(read);
        self
    }

    /// Sets the option for write access.
    ///
    /// This option, when true, will allow the file to be written to.
    /// Defaults to `false`.
    pub fn write(&mut self, write: bool) -> &mut Self {
        self.flags.set_write(write);
        self
    }

    /// Sets the option for append mode.
    ///
    /// This option, when true, means that writes will append to the file instead
    /// of overwriting previous content.
    /// Defaults to `false`.
    pub fn append(&mut self, append: bool) -> &mut Self {
        self.flags.set_append(append);
        self
    }

    /// Sets the option to truncate the file.
    ///
    /// When true, if the file exists and is opened for writing, it will be truncated
    /// to 0 length.
    /// Defaults to `false`.
    pub fn truncate(&mut self, truncate: bool) -> &mut Self {
        self.flags.set_truncate(truncate);
        self
    }

    /// Sets the option to create a new file if it does not exist.
    ///
    /// When true, a new file will be created if `path` does not exist.
    /// Defaults to `false`.
    pub fn create(&mut self, create: bool) -> &mut Self {
        self.flags.set_create(create);
        self
    }

    /// Sets the option to create a new file, failing if it already exists.
    ///
    /// When true, a new file will be created, but the operation will fail if
    /// a file at `path` already exists.
    /// Defaults to `false`.
    pub fn create_new(&mut self, create_new: bool) -> &mut Self {
        self.flags.set_create_new(create_new);
        self
    }

    /// Opens the file at `path` with the options specified for this builder.
    pub async fn open<P>(&self, path: &P) -> io::Result<AtomicDirFile>
    where
        P: AsRef<Path> + ?Sized,
    {
        let file = self
            .parent
            .open_file(path.as_ref(), self.flags.clone())
            .await?;
        Ok(file)
    }
}

#[derive(Clone)]
struct Inner {
    engine: Arc<Engine<TokioFileSystemOperations>>,
    open_tracker: OpenTracker,
}

impl Inner {
    fn new(engine: Engine<TokioFileSystemOperations>) -> Self {
        Self {
            engine: Arc::new(engine),
            open_tracker: OpenTracker::new(),
        }
    }
    async fn open_file(&self, path: &Path, options: OpenOptionsFlags) -> io::Result<AtomicDirFile> {
        Ok(AtomicDirFile::new(
            self.engine.open_file(path, &options).await?,
            WrapperTracker::new(self.engine.clone(), self.open_tracker.spawn_marker()),
        ))
    }

    fn into_engine(self) -> Option<Engine<TokioFileSystemOperations>> {
        Arc::into_inner(self.engine)
    }

    async fn commit(self) -> io::Result<()> {
        self.open_tracker.wait_for_close().await;
        self.into_engine().unwrap().commit().await
    }
}

/// A handle to an atomic directory.
///
/// This provides a cloneable reference to an `AtomicDir`, allowing multiple
/// parts of a program to share access to the same atomic directory instance.
/// It provides a subset of the functionality of `AtomicDir`, primarily focused
/// on operating on files within the directory.
#[derive(Clone)]
pub struct Handle {
    inner: Inner,
    _marker: OpenMarker,
}

impl Handle {
    #[must_use]
    pub fn open_options(&self) -> OpenOptions<'_> {
        OpenOptions {
            parent: &self.inner,
            flags: OpenOptionsFlags::default(),
        }
    }
}

/// A high-level atomic directory that allows for changing files within
/// a directory atomically.
///
/// This guarantees that either all changes are applied, or none are, even in
/// the case of crashes or interruptions. Changes are staged in a temporary
/// directory and then committed atomically. If the program crashes, a recovery
/// process will attempt to complete the commit the next time an `AtomicDir`
/// is created for the same directory.
pub struct AtomicDir {
    inner: Option<Inner>,
}

impl AtomicDir {
    fn get_inner(&self) -> &Arc<Engine<TokioFileSystemOperations>> {
        &self.inner.as_ref().unwrap().engine
    }

    fn take_inner(&mut self) -> Option<Engine<TokioFileSystemOperations>> {
        self.inner
            .take()
            .expect("AtomicDir has already been consumed")
            .into_engine()
    }

    /// Creates a new `AtomicDir` at the specified directory path.
    ///
    /// This will acquire an exclusive lock on the directory, preventing other
    /// `AtomicDir` instances from operating on it. If a previous, incomplete
    /// commit is detected, this will attempt to recover it.
    pub async fn new_at_dir<P>(dir_root: &P) -> io::Result<Self>
    where
        P: AsRef<Path> + ?Sized,
    {
        let engine = Engine::create_at_dir(TokioFileSystemOperations, dir_root.as_ref()).await?;
        Ok(AtomicDir {
            inner: Some(Inner::new(engine)),
        })
    }

    /// Tries to create a new `AtomicDir` at the specified directory path.
    ///
    /// This is a non-blocking version of `new_at_dir`. If the directory is
    /// already locked, it will return `Ok(None)` instead of waiting.
    pub async fn try_new_at_dir<P>(dir_root: &P) -> io::Result<Option<Self>>
    where
        P: AsRef<Path> + ?Sized,
    {
        let Some(engine) =
            Engine::try_create_at_dir(TokioFileSystemOperations, dir_root.as_ref()).await?
        else {
            return Ok(None);
        };
        Ok(Some(AtomicDir {
            inner: Some(Inner::new(engine)),
        }))
    }

    #[must_use]
    pub fn as_handle(&self) -> Handle {
        let inner = self.inner.as_ref().unwrap();
        Handle {
            inner: inner.clone(),
            _marker: inner.open_tracker.spawn_marker(),
        }
    }

    /// Returns a new `OpenOptions` builder for opening files within this `AtomicDir`.
    #[must_use]
    pub fn open_options(&self) -> OpenOptions<'_> {
        OpenOptions {
            parent: self.inner.as_ref().unwrap(),
            flags: OpenOptionsFlags::default(),
        }
    }

    /// Deletes a file within the atomic directory transaction.
    ///
    /// The deletion is staged and will be finalized upon `commit`.
    pub async fn delete<'a, P>(&'a self, path: &'a P) -> io::Result<()>
    where
        P: AsRef<Path> + ?Sized,
    {
        self.get_inner().delete_path(path.as_ref()).await
    }

    pub async fn list_dir<'a, P>(
        &'a self,
        path: &'a P,
    ) -> io::Result<impl TryStream<Ok = DirEntry, Error = io::Error> + Unpin + 'a>
    where
        P: AsRef<Path> + ?Sized,
    {
        self.get_inner().list_dir(path.as_ref()).await
    }

    pub async fn exists<P>(&self, path: &P) -> io::Result<bool>
    where
        P: AsRef<Path> + ?Sized,
    {
        self.get_inner().exists(path.as_ref()).await
    }

    /// Commits all staged changes to the directory.
    ///
    /// This makes all writes and deletes permanent and visible to other processes.
    /// The commit itself is an atomic operation. If it is interrupted, it will
    /// be completed the next time an `AtomicDir` is created for this directory.
    pub async fn commit(mut self) -> io::Result<()> {
        self.inner.take().unwrap().commit().await
    }

    pub async fn abort(mut self) -> io::Result<()> {
        // If we aren't the last reference, we will abort when the last
        // reference is dropped.
        if let Some(inner) = self.take_inner() {
            inner.abort().await
        } else {
            Ok(())
        }
    }

    pub async fn metadata<P>(&self, path: &P) -> io::Result<Metadata>
    where
        P: AsRef<Path> + ?Sized,
    {
        self.get_inner().metadata(path.as_ref()).await
    }
}

/// Helper functions
impl AtomicDir {
    /// A convenience method to write data to a file within the transaction.
    ///
    /// This is equivalent to using `open_options` to open a file for writing
    /// and then writing the data.
    pub async fn write<P>(&self, path: &P, write_mode: WriteMode, data: &[u8]) -> io::Result<()>
    where
        P: AsRef<Path> + ?Sized,
    {
        let mut options = self.open_options();
        match write_mode {
            WriteMode::CreateNew => {
                options.create_new(true);
            }
            WriteMode::Overwrite => {
                options.create(true).truncate(true);
            }
        }
        let mut file = options.write(true).open(path.as_ref()).await?;
        file.write_all(data).await?;
        Ok(())
    }

    /// A convenience method to read data from a file within the transaction.
    ///
    /// This reads the entire contents of the file into a `Vec<u8>`.
    pub async fn read<P>(&self, path: &P) -> io::Result<Vec<u8>>
    where
        P: AsRef<Path> + ?Sized,
    {
        let mut options = self.open_options();
        options.read(true);
        let mut file = options.open(path).await?;
        let mut data = Vec::new();
        file.read_to_end(&mut data).await?;
        Ok(data)
    }
}

impl Drop for AtomicDir {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.take()
            && let Some(inner) = inner.into_engine()
        {
            // We have not been committed, so we should abort the transaction.
            // We do this in a background task to avoid blocking the drop.
            tokio::spawn(inner.abort());
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::fs::{
        atomic_dir::schema::{CommitEntry, CommitSchema, DeleteEntry, OverwriteEntry},
        paths::RelPathBuf,
    };

    use super::*;
    use futures::TryStreamExt as _;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_write_and_commit() -> io::Result<()> {
        let dir = tempdir()?;

        let atomic_dir = AtomicDir::new_at_dir(dir.path()).await?;
        atomic_dir
            .write("foo.txt", WriteMode::CreateNew, b"hello")
            .await?;

        atomic_dir.commit().await?;

        let contents = tokio::fs::read_to_string(&dir.path().join("foo.txt")).await?;
        assert_eq!(contents, "hello");

        Ok(())
    }

    #[tokio::test]
    async fn test_delete_and_commit() -> io::Result<()> {
        let dir = tempdir()?;

        // Create a file to be deleted.
        tokio::fs::write(dir.path().join("foo.txt"), "hello").await?;

        let atomic_dir = AtomicDir::new_at_dir(dir.path()).await?;
        atomic_dir.delete(Path::new("foo.txt")).await?;
        atomic_dir.commit().await?;

        assert!(!dir.path().join("foo.txt").exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_write_delete_and_commit() -> io::Result<()> {
        let dir = tempdir()?;

        // Create a file to be overwritten.
        tokio::fs::write(dir.path().join("foo.txt"), "old content").await?;

        let atomic_dir = AtomicDir::new_at_dir(dir.path()).await?;
        atomic_dir
            .write(Path::new("foo.txt"), WriteMode::Overwrite, b"new content")
            .await?;
        atomic_dir.delete(Path::new("bar.txt")).await?;
        atomic_dir
            .write(Path::new("bar.txt"), WriteMode::CreateNew, b"new file")
            .await?;
        atomic_dir.commit().await?;

        let contents = tokio::fs::read_to_string(&dir.path().join("foo.txt")).await?;
        assert_eq!(contents, "new content");
        let contents_bar = tokio::fs::read_to_string(&dir.path().join("bar.txt")).await?;
        assert_eq!(contents_bar, "new file");

        Ok(())
    }

    #[tokio::test]
    async fn test_list_dir_reflects_staged_state() -> io::Result<()> {
        let dir = tempdir()?;
        let assets_dir = dir.path().join("assets");
        tokio::fs::create_dir(&assets_dir).await?;

        tokio::fs::write(assets_dir.join("existing.txt"), b"old").await?;
        tokio::fs::write(assets_dir.join("deleted.txt"), b"remove me").await?;
        tokio::fs::write(assets_dir.join("untouched.txt"), b"keep").await?;

        let atomic_dir = AtomicDir::new_at_dir(dir.path()).await?;

        atomic_dir
            .write(Path::new("assets/new.txt"), WriteMode::CreateNew, b"new")
            .await?;
        atomic_dir
            .write(
                Path::new("assets/existing.txt"),
                WriteMode::Overwrite,
                b"updated",
            )
            .await?;
        atomic_dir.delete(Path::new("assets/deleted.txt")).await?;

        let mut entries = atomic_dir.list_dir(Path::new("assets")).await?;
        let mut observed = std::collections::BTreeSet::new();

        while let Some(entry) = entries.try_next().await? {
            assert!(entry.file_type().is_file());
            let rel = entry.file_name().to_string_lossy().into_owned();
            observed.insert(rel);
        }

        let expected: std::collections::BTreeSet<String> =
            ["existing.txt", "new.txt", "untouched.txt"]
                .into_iter()
                .map(String::from)
                .collect();

        assert_eq!(observed, expected);

        Ok(())
    }

    #[tokio::test]
    async fn test_recovery() -> io::Result<()> {
        let dir = tempdir()?;

        // Simulate a partial commit.
        let commit_schema = CommitSchema::new(
            RelPathBuf::new_checked("tmpdir-recovery-test").unwrap(),
            vec![
                CommitEntry::Overwrite(OverwriteEntry::new(
                    RelPathBuf::new_checked("foo.txt").unwrap(),
                )),
                CommitEntry::Delete(DeleteEntry::new(
                    RelPathBuf::new_checked("bar.txt").unwrap(),
                )),
            ],
        );

        // Create the temporary directory and file.
        tokio::fs::create_dir(dir.path().join("tmpdir-recovery-test")).await?;
        tokio::fs::write(dir.path().join("tmpdir-recovery-test/foo.txt"), "recovered").await?;

        // Create a file to be deleted.
        tokio::fs::write(dir.path().join("bar.txt"), "to be deleted").await?;

        // Write the commit file.
        let commit_data = serde_json::to_vec(&commit_schema)?;
        tokio::fs::write(dir.path().join(COMMIT_PATH), commit_data).await?;

        // Now, run recovery.
        let _atomic_dir = AtomicDir::new_at_dir(dir.path()).await?;

        // Check that recovery happened.
        let contents = tokio::fs::read_to_string(&dir.path().join("foo.txt")).await?;
        assert_eq!(contents, "recovered");
        assert!(!dir.path().join("bar.txt").exists());
        assert!(!dir.path().join(COMMIT_PATH).exists());
        assert!(!dir.path().join("tmpdir-recovery-test").exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_dir_locking() -> io::Result<()> {
        let dir = tempdir()?;

        {
            // Acquire a lock by creating an AtomicDirInner.
            let _atomic_dir1 = AtomicDir::new_at_dir(dir.path()).await?;

            // Try to acquire another lock on the same directory.
            // This should fail with `Ok(None)` because the first one is still held.
            let atomic_dir2 = AtomicDir::try_new_at_dir(dir.path()).await?;
            assert!(
                atomic_dir2.is_none(),
                "Should not be able to acquire lock while it's held"
            );
        } // atomic_dir1 is dropped here, releasing the lock.

        // Now that the first lock is released, we should be able to acquire a new one.
        let atomic_dir3 = AtomicDir::try_new_at_dir(dir.path()).await?;
        assert!(
            atomic_dir3.is_some(),
            "Should be able to acquire lock after it's released"
        );

        Ok(())
    }
}
