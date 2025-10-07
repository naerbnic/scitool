//! Atomic directory implementation.

mod commit;
mod dir_lock;
mod dir_state;
mod new_engine;
mod recovery;
mod types;
mod util;

use std::{
    ffi::OsString,
    io::{self, Read, Seek as _, Write},
    os::fd::{AsFd, AsRawFd},
    path::{Path, PathBuf},
};

use cap_std::fs::Dir;
use serde::Serialize;

pub use self::types::FileType;
pub use crate::fs::ops::WriteMode;
use crate::fs::{
    atomic_dir::{
        commit::CommitFileData,
        dir_lock::DirLock,
        dir_state::{DirState, LoadedDirState},
        recovery::{recover, recover_exclusive},
        util::{create_old_path, create_tmp_path},
    },
    err_helpers::io_bail,
    file_lock::LockType,
    paths::RelPathBuf,
};

const STATE_FILE_NAME: &str = ".state";

struct ReadOnlyHandle {}

pub struct File {
    inner: cap_std::fs::File,
}

impl File {
    fn from_inner(inner: cap_std::fs::File) -> Self {
        File { inner }
    }
    /// Attempts to sync all OS-internal metadata to disk.
    ///
    /// This corresponds to [`std::fs::File::sync_all`].
    #[inline]
    pub fn sync_all(&self) -> io::Result<()> {
        self.inner.sync_all()
    }

    /// This function is similar to `sync_all`, except that it may not
    /// synchronize file metadata to a filesystem.
    ///
    /// This corresponds to [`std::fs::File::sync_data`].
    #[inline]
    pub fn sync_data(&self) -> io::Result<()> {
        self.inner.sync_data()
    }

    /// Truncates or extends the underlying file, updating the size of this
    /// file to become size.
    ///
    /// This corresponds to [`std::fs::File::set_len`].
    #[inline]
    pub fn set_len(&self, size: u64) -> io::Result<()> {
        self.inner.set_len(size)
    }
}

impl Read for File {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

impl Read for &File {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (&mut &self.inner).read(buf)
    }
}

impl Write for File {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl Write for &File {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (&mut &self.inner).write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        (&mut &self.inner).flush()
    }
}

#[cfg(not(windows))]
impl AsRawFd for File {
    fn as_raw_fd(&self) -> std::os::unix::io::RawFd {
        self.inner.as_raw_fd()
    }
}

#[cfg(not(windows))]
impl AsFd for &File {
    fn as_fd(&self) -> std::os::unix::prelude::BorrowedFd<'_> {
        self.inner.as_fd()
    }
}

pub struct OpenOptions {
    inner: cap_std::fs::OpenOptions,
}

impl OpenOptions {
    pub fn new() -> Self {
        OpenOptions {
            inner: cap_std::fs::OpenOptions::new(),
        }
    }

    pub fn read(&mut self, read: bool) -> &mut Self {
        self.inner.read(read);
        self
    }

    pub fn write(&mut self, write: bool) -> &mut Self {
        self.inner.write(write);
        self
    }

    pub fn append(&mut self, append: bool) -> &mut Self {
        self.inner.append(append);
        self
    }

    pub fn truncate(&mut self, truncate: bool) -> &mut Self {
        self.inner.truncate(truncate);
        self
    }

    pub fn create(&mut self, create: bool) -> &mut Self {
        self.inner.create(create);
        self
    }

    pub fn create_new(&mut self, create_new: bool) -> &mut Self {
        self.inner.create_new(create_new);
        self
    }
}

pub struct ReadDir {
    inner: cap_std::fs::ReadDir,
}

impl Iterator for ReadDir {
    type Item = io::Result<DirEntry>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|res| res.map(DirEntry::from_inner))
    }
}

pub struct DirEntry {
    inner: cap_std::fs::DirEntry,
}

impl DirEntry {
    fn from_inner(inner: cap_std::fs::DirEntry) -> Self {
        DirEntry { inner }
    }

    /// Open the file for reading.
    #[inline]
    pub fn open(&self) -> io::Result<File> {
        let file = self.inner.open()?;
        Ok(File { inner: file })
    }

    /// Open the file with the given options.
    #[inline]
    pub fn open_with(&self, options: &OpenOptions) -> io::Result<File> {
        let file = self.inner.open_with(&options.inner)?;
        Ok(File { inner: file })
    }

    /// Removes the file from its filesystem.
    #[inline]
    pub fn remove_file(&self) -> io::Result<()> {
        self.inner.remove_file()
    }

    /// Removes the directory from its filesystem.
    #[inline]
    pub fn remove_dir(&self) -> io::Result<()> {
        self.inner.remove_dir()
    }

    /// Returns the metadata for the file that this entry points at.
    ///
    /// This corresponds to [`std::fs::DirEntry::metadata`].
    #[inline]
    pub fn metadata(&self) -> io::Result<Metadata> {
        Ok(Metadata {
            inner: self.inner.metadata()?,
        })
    }

    /// Returns the file type for the file that this entry points at.
    ///
    /// This corresponds to [`std::fs::DirEntry::file_type`].
    #[inline]
    pub fn file_type(&self) -> io::Result<FileType> {
        Ok(FileType::of_cap_std(self.inner.file_type()?))
    }

    /// Returns the bare file name of this directory entry without any other
    /// leading path component.
    ///
    /// This corresponds to [`std::fs::DirEntry::file_name`].
    #[inline]
    pub fn file_name(&self) -> OsString {
        self.inner.file_name()
    }
}

pub struct Metadata {
    inner: cap_std::fs::Metadata,
}

fn open_dir_safe(path: &Path, lock_type: LockType) -> io::Result<DirLock> {
    let mut target_lock = DirLock::acquire(path, lock_type)?;
    target_lock = recover(target_lock)?;

    Ok(target_lock)
}

fn read_state_file(dir: &Dir) -> io::Result<LoadedDirState> {
    let mut state_file =
        dir.open_with(STATE_FILE_NAME, cap_std::fs::OpenOptions::new().read(true))?;
    let state_contents = {
        let mut buf = Vec::new();
        state_file.read_to_end(&mut buf)?;
        buf
    };

    Ok(DirState::load(&state_contents)?)
}

struct Inner {
    lock: DirLock,
    dir_handle: Dir,
    state: DirState,
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
    inner: Inner,
}

impl AtomicDir {
    fn from_lock(lock: DirLock) -> io::Result<Self> {
        let dir_handle = Dir::open_ambient_dir(lock.path(), cap_std::ambient_authority())?;

        let LoadedDirState::Clean(state) = read_state_file(&dir_handle)? else {
            io_bail!(
                InvalidData,
                "Directory has been poisoned due to previous failed operation. This is an inconsistent state: {}",
                lock.path().display()
            );
        };

        Ok(AtomicDir {
            inner: Inner {
                lock,
                dir_handle,
                state,
            },
        })
    }

    pub fn open<P>(path: &P) -> io::Result<Self>
    where
        P: AsRef<Path> + ?Sized,
    {
        Self::from_lock(open_dir_safe(path.as_ref(), LockType::Shared)?)
    }

    pub fn open_file(&self, path: &Path) -> io::Result<File> {
        let path = util::normalize_path(path)?;
        Ok(File {
            inner: self
                .inner
                .dir_handle
                .open_with(&path, cap_std::fs::OpenOptions::new().read(true))?,
        })
    }

    pub fn read_dir<P>(&self, path: &P) -> io::Result<ReadDir>
    where
        P: AsRef<Path> + ?Sized,
    {
        Ok(ReadDir {
            inner: self.inner.dir_handle.read_dir(path.as_ref())?,
        })
    }

    pub fn metadata<P>(&self, path: &P) -> io::Result<Metadata>
    where
        P: AsRef<Path> + ?Sized,
    {
        let path = util::normalize_path(path.as_ref())?;
        Ok(Metadata {
            inner: self.inner.dir_handle.metadata(&path)?,
        })
    }

    pub fn begin_update(&self) -> io::Result<DirBuilder> {
        todo!()
    }
}

/// Helper functions
impl AtomicDir {
    /// A convenience method to read data from a file within the transaction.
    ///
    /// This reads the entire contents of the file into a `Vec<u8>`.
    pub async fn read<P>(&self, path: &P) -> io::Result<Vec<u8>>
    where
        P: AsRef<Path> + ?Sized,
    {
        let mut file = self.open_file(path.as_ref())?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;
        Ok(buf)
    }
}

struct TempDir {
    
}

enum SourceDir {
    Existing(AtomicDir),
    New(DirLock),
}

impl SourceDir {
    fn lock(&self) -> &DirLock {
        match self {
            SourceDir::Existing(dir) => &dir.inner.lock,
            SourceDir::New(lock) => lock,
        }
    }
}

/// A builder to create a new `AtomicDir`, or overwrite an existing one.
pub struct DirBuilder {
    source: SourceDir,
    curr_state: DirState,
    temp_path: Option<RelPathBuf>,
    temp_dir: Dir,
}

impl DirBuilder {
    pub fn new_empty<P>(path: &P) -> io::Result<Self>
    where
        P: AsRef<Path> + ?Sized,
    {
        let target_lock = open_dir_safe(path.as_ref(), LockType::Exclusive)?;
        match cap_std::fs::Dir::open_ambient_dir(path, cap_std::ambient_authority()) {
            Ok(_) => {
                // We should have nothing at the target path, or at the commit file for the path. Otherwise,
                // we might be overwriting an existing directory.
                io_bail!(
                    AlreadyExists,
                    "Target path already exists: {}",
                    target_lock.path().display()
                )
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }

        // Create a temporary directory within the parent of the target path.
        let temp_path = create_tmp_path(&target_lock);
        let temp_dir = target_lock.parent_dir().open_dir(&temp_path)?;
        Ok(DirBuilder {
            source: SourceDir::New(target_lock),
            curr_state: DirState::new(),
            temp_path: Some(temp_path),
            temp_dir,
        })
    }

    fn from_atomic_dir(dir: AtomicDir) -> io::Result<Self> {
        let temp_path = create_tmp_path(&dir.inner.lock);
        let temp_dir = dir.inner.lock.parent_dir().open_dir(&temp_path)?;
        Ok(DirBuilder {
            source: SourceDir::Existing(dir),
            curr_state: DirState::new(),
            temp_path: Some(temp_path),
            temp_dir,
        })
    }

    pub fn source(&self) -> Option<&AtomicDir> {
        match &self.source {
            SourceDir::Existing(dir) => Some(dir),
            SourceDir::New(_) => None,
        }
    }

    pub fn copy_file(&self, src: &Path, dst: &Path) -> io::Result<()> {
        let SourceDir::Existing(source_dir) = &self.source else {
            io_bail!(
                Other,
                "Can only copy files when updating an existing directory"
            );
        };
        let dst = self.temp_dir.canonicalize(dst)?;
        if let Some(parent) = dst.parent() {
            self.temp_dir.create_dir_all(parent)?;
        }
        source_dir
            .inner
            .dir_handle
            .hard_link(&src, &self.temp_dir, &dst)?;
        Ok(())
    }

    pub fn copy_for_writing(&self, src: &Path, dst: &Path) -> io::Result<File> {
        let SourceDir::Existing(source_dir) = &self.source else {
            io_bail!(
                Other,
                "Can only copy files when updating an existing directory"
            );
        };
        let dst = self.temp_dir.canonicalize(dst)?;
        if let Some(parent) = dst.parent() {
            self.temp_dir.create_dir_all(parent)?;
        }
        let source_file = source_dir
            .inner
            .dir_handle
            .open_with(&src, cap_std::fs::OpenOptions::new().read(true))?;

        let mut dest_file = self.temp_dir.open_with(
            &dst,
            cap_std::fs::OpenOptions::new().write(true).create_new(true),
        )?;

        std::io::copy(&mut &source_file, &mut &dest_file)?;
        drop(source_file);
        dest_file.seek(std::io::SeekFrom::Start(0))?;
        dest_file.sync_all()?;
        Ok(File { inner: dest_file })
    }

    pub fn write_file(&self, path: &Path, data: &[u8]) -> io::Result<()> {
        let path = self.temp_dir.canonicalize(path)?;
        if let Some(parent) = path.parent() {
            self.temp_dir.create_dir_all(parent)?;
        }
        // We don't need to worry about atomicity here, since the entire temp directory
        // will be moved into place atomically during commit, or deleted during abort.
        {
            let mut file = self.temp_dir.open_with(
                &path,
                cap_std::fs::OpenOptions::new().write(true).create_new(true),
            )?;
            file.write_all(data)?;
            file.sync_all()?;
        }
        Ok(())
    }

    pub fn remove_file(&self, path: &Path) -> io::Result<()> {
        let path = self.temp_dir.canonicalize(path)?;
        self.temp_dir.remove_file(&path)?;
        Ok(())
    }

    pub fn read_dir(&self, path: &Path) -> io::Result<ReadDir> {
        let path = self.temp_dir.canonicalize(path)?;
        Ok(ReadDir {
            inner: self.temp_dir.read_dir(&path)?,
        })
    }

    pub fn commit(self) -> io::Result<AtomicDir> {
        let target_lock = match self.source {
            SourceDir::Existing(dir) => dir.inner.lock,
            SourceDir::New(lock) => lock,
        };
        let mut target_lock = target_lock.upgrade()?;
        {
            let mut state_file = std::fs::File::options()
                .write(true)
                .create(true)
                .truncate(true)
                .open(target_lock.path().join(STATE_FILE_NAME))?;
            state_file.write_all(&self.curr_state.to_next().serialize()?)?;
            state_file.flush()?;
            state_file.sync_all()?;
        }
        let temp_path = self
            .temp_path
            .as_ref()
            .expect("Temp path must exist")
            .clone();
        let commit = CommitFileData::new(
            temp_path.clone().into_path_buf(),
            create_old_path(&target_lock).into_path_buf(),
        );

        if let Err(e) = commit.commit_file(&target_lock) {
            // Failed to write the commit file. Try to clean up after ourselves.
            drop(target_lock.parent_dir().remove_dir_all(&temp_path));
            return Err(e);
        }

        // Now, perform the recovery steps to move the temp directory into place.
        //
        // Even if this fails, opening the directory again will recover it.
        recover_exclusive(&target_lock)?;

        target_lock = target_lock.downgrade()?;

        target_lock = recover(target_lock)?;

        AtomicDir::from_lock(target_lock)
    }
}

// #[cfg(test)]
// mod tests {
//     use crate::fs::{
//         atomic_dir::schema::{CommitEntry, CommitSchema, DeleteEntry, OverwriteEntry},
//         paths::RelPathBuf,
//     };

//     use super::*;
//     use tempfile::tempdir;

//     #[test]
//     fn test_write_and_commit() -> io::Result<()> {
//         let dir = tempdir()?;

//         let atomic_dir = AtomicDir::new_at_dir(dir.path())?;
//         atomic_dir.write("foo.txt", &WriteMode::CreateNew, b"hello")?;

//         atomic_dir.commit()?;

//         let contents = std::fs::read_to_string(dir.path().join("foo.txt"))?;
//         assert_eq!(contents, "hello");

//         Ok(())
//     }

//     #[test]
//     fn test_delete_and_commit() -> io::Result<()> {
//         let dir = tempdir()?;

//         // Create a file to be deleted.
//         std::fs::write(dir.path().join("foo.txt"), "hello")?;

//         let atomic_dir = AtomicDir::new_at_dir(dir.path())?;
//         atomic_dir.delete(Path::new("foo.txt"))?;
//         atomic_dir.commit()?;

//         assert!(!dir.path().join("foo.txt").exists());

//         Ok(())
//     }

//     #[test]
//     fn test_write_delete_and_commit() -> io::Result<()> {
//         let dir = tempdir()?;

//         // Create a file to be overwritten.
//         std::fs::write(dir.path().join("foo.txt"), "old content")?;

//         let atomic_dir = AtomicDir::new_at_dir(dir.path())?;
//         atomic_dir.write(Path::new("foo.txt"), &WriteMode::Overwrite, b"new content")?;
//         atomic_dir.delete(Path::new("bar.txt"))?;
//         atomic_dir.write(Path::new("bar.txt"), &WriteMode::CreateNew, b"new file")?;
//         atomic_dir.commit()?;

//         let contents = std::fs::read_to_string(dir.path().join("foo.txt"))?;
//         assert_eq!(contents, "new content");
//         let contents_bar = std::fs::read_to_string(dir.path().join("bar.txt"))?;
//         assert_eq!(contents_bar, "new file");

//         Ok(())
//     }

//     #[test]
//     fn test_list_dir_reflects_staged_state() -> io::Result<()> {
//         let dir = tempdir()?;
//         let assets_dir = dir.path().join("assets");
//         std::fs::create_dir(&assets_dir)?;

//         std::fs::write(assets_dir.join("existing.txt"), b"old")?;
//         std::fs::write(assets_dir.join("deleted.txt"), b"remove me")?;
//         std::fs::write(assets_dir.join("untouched.txt"), b"keep")?;

//         let atomic_dir = AtomicDir::new_at_dir(dir.path())?;

//         atomic_dir.write(Path::new("assets/new.txt"), &WriteMode::CreateNew, b"new")?;
//         atomic_dir.write(
//             Path::new("assets/existing.txt"),
//             &WriteMode::Overwrite,
//             b"updated",
//         )?;
//         atomic_dir.delete(Path::new("assets/deleted.txt"))?;

//         let entries = atomic_dir.list_dir(Path::new("assets"))?;
//         let mut observed = std::collections::BTreeSet::new();

//         for entry in entries {
//             let entry = entry?;
//             assert!(entry.file_type().is_file());
//             let rel = entry.file_name().to_string_lossy().into_owned();
//             observed.insert(rel);
//         }

//         let expected: std::collections::BTreeSet<String> =
//             ["existing.txt", "new.txt", "untouched.txt"]
//                 .into_iter()
//                 .map(String::from)
//                 .collect();

//         assert_eq!(observed, expected);

//         Ok(())
//     }

//     #[test]
//     fn test_recovery() -> io::Result<()> {
//         let dir = tempdir()?;

//         // Simulate a partial commit.
//         let commit_schema = CommitSchema::new(
//             RelPathBuf::new_checked("tmpdir-recovery-test").unwrap(),
//             vec![
//                 CommitEntry::Overwrite(OverwriteEntry::new(
//                     RelPathBuf::new_checked("foo.txt").unwrap(),
//                 )),
//                 CommitEntry::Delete(DeleteEntry::new(
//                     RelPathBuf::new_checked("bar.txt").unwrap(),
//                 )),
//             ],
//         );

//         // Create the temporary directory and file.
//         std::fs::create_dir(dir.path().join("tmpdir-recovery-test"))?;
//         std::fs::write(dir.path().join("tmpdir-recovery-test/foo.txt"), "recovered")?;

//         // Create a file to be deleted.
//         std::fs::write(dir.path().join("bar.txt"), "to be deleted")?;

//         // Write the commit file.
//         let commit_data = serde_json::to_vec(&commit_schema)?;
//         std::fs::write(dir.path().join(COMMIT_PATH), commit_data)?;

//         // Now, run recovery.
//         let _atomic_dir = AtomicDir::new_at_dir(dir.path())?;

//         // Check that recovery happened.
//         let contents = std::fs::read_to_string(dir.path().join("foo.txt"))?;
//         assert_eq!(contents, "recovered");
//         assert!(!dir.path().join("bar.txt").exists());
//         assert!(!dir.path().join(COMMIT_PATH).exists());
//         assert!(!dir.path().join("tmpdir-recovery-test").exists());

//         Ok(())
//     }

//     #[test]
//     fn test_dir_locking() -> io::Result<()> {
//         let dir = tempdir()?;

//         {
//             // Acquire a lock by creating an AtomicDirInner.
//             let _atomic_dir1 = AtomicDir::new_at_dir(dir.path())?;

//             // Try to acquire another lock on the same directory.
//             // This should fail with `Ok(None)` because the first one is still held.
//             let atomic_dir2 = AtomicDir::try_new_at_dir(dir.path())?;
//             assert!(
//                 atomic_dir2.is_none(),
//                 "Should not be able to acquire lock while it's held"
//             );
//         } // atomic_dir1 is dropped here, releasing the lock.

//         // Now that the first lock is released, we should be able to acquire a new one.
//         let atomic_dir3 = AtomicDir::try_new_at_dir(dir.path())?;
//         assert!(
//             atomic_dir3.is_some(),
//             "Should be able to acquire lock after it's released"
//         );

//         Ok(())
//     }
// }
