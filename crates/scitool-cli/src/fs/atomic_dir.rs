//! Atomic directory implementation.

mod commit;
mod dir_lock;
mod dir_state;
mod new_engine;
mod recovery;
mod temp_dir;
mod types;
mod util;

use std::{
    borrow::Cow,
    ffi::OsString,
    io::{self, Read, Seek as _, Write},
    os::fd::{AsFd, AsRawFd},
    path::{Path, PathBuf},
    sync::Arc,
};

use cap_std::fs::Dir;
use rand::{Rng as _, distr::SampleString as _};
use serde::Serialize;

pub use self::types::FileType;
pub use crate::fs::ops::WriteMode;
use crate::fs::{
    atomic_dir::{
        commit::CommitFileData,
        dir_lock::DirLock,
        dir_state::{DirState, LoadedDirState},
        recovery::{recover, recover_exclusive},
        temp_dir::TempDir,
        util::create_old_path,
    },
    err_helpers::{io_bail, io_err_map},
    file_lock::LockType,
    paths::{RelPathBuf, SinglePath, SinglePathBuf},
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
    at_root: bool,
    inner: cap_std::fs::ReadDir,
}

impl Iterator for ReadDir {
    type Item = io::Result<DirEntry>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next()? {
                Ok(entry) => {
                    if !self.at_root || entry.file_name() != OsString::from(STATE_FILE_NAME) {
                        return Some(Ok(DirEntry::from_inner(entry)));
                    }
                }
                Err(err) => return Some(Err(err)),
            }
        }
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
    #[must_use]
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

/// A high-level atomic directory that allows for changing files within
/// a directory atomically.
///
/// This guarantees that either all changes are applied, or none are, even in
/// the case of crashes or interruptions. Changes are staged in a temporary
/// directory and then committed atomically. If the program crashes, a recovery
/// process will attempt to complete the commit the next time an `AtomicDir`
/// is created for the same directory.
pub struct AtomicDir {
    lock: DirLock,
    dir_handle: Dir,
    state: DirState,
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
            lock,
            dir_handle,
            state,
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
                .dir_handle
                .open_with(&path, cap_std::fs::OpenOptions::new().read(true))?,
        })
    }

    pub fn read_dir<P>(&self, path: &P) -> io::Result<ReadDir>
    where
        P: AsRef<Path> + ?Sized,
    {
        let path = self.dir_handle.canonicalize(path.as_ref())?;
        Ok(ReadDir {
            at_root: path.as_os_str() == "." || path.as_os_str() == "",
            inner: self.dir_handle.read_dir(path)?,
        })
    }

    pub fn metadata<P>(&self, path: &P) -> io::Result<Metadata>
    where
        P: AsRef<Path> + ?Sized,
    {
        let path = util::normalize_path(path.as_ref())?;
        Ok(Metadata {
            inner: self.dir_handle.metadata(&path)?,
        })
    }

    pub fn begin_update(self) -> io::Result<DirBuilder> {
        DirBuilder::from_atomic_dir(self)
    }
}

/// Helper functions
impl AtomicDir {
    /// A convenience method to read data from a file within the transaction.
    ///
    /// This reads the entire contents of the file into a `Vec<u8>`.
    pub fn read<P>(&self, path: &P) -> io::Result<Vec<u8>>
    where
        P: AsRef<Path> + ?Sized,
    {
        let mut file = self.open_file(path.as_ref())?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;
        Ok(buf)
    }
}

enum SourceDir {
    Existing(AtomicDir),
    New(DirLock),
}

/// A builder to create a new `AtomicDir`, or overwrite an existing one.
pub struct DirBuilder {
    source: SourceDir,
    temp_dir: TempDir,
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
        let temp_dir = TempDir::new_in(target_lock.parent_dir().clone(), target_lock.file_name())?;
        Ok(DirBuilder {
            source: SourceDir::New(target_lock),
            temp_dir,
        })
    }

    fn from_atomic_dir(dir: AtomicDir) -> io::Result<Self> {
        let temp_dir = TempDir::new_in(dir.lock.parent_dir().clone(), dir.lock.file_name())?;
        Ok(DirBuilder {
            source: SourceDir::Existing(dir),
            temp_dir,
        })
    }

    #[must_use]
    pub fn source(&self) -> Option<&AtomicDir> {
        match &self.source {
            SourceDir::Existing(dir) => Some(dir),
            SourceDir::New(_) => None,
        }
    }

    pub fn copy_file<P1, P2>(&self, src: &P1, dst: &P2) -> io::Result<()>
    where
        P1: AsRef<Path> + ?Sized,
        P2: AsRef<Path> + ?Sized,
    {
        let SourceDir::Existing(source_dir) = &self.source else {
            io_bail!(
                Other,
                "Can only copy files when updating an existing directory"
            );
        };
        let Some((dst_parent, dst_file_name)) = util::safe_path_parent(dst.as_ref())? else {
            io_bail!(
                Other,
                "Destination path must have a parent: {}",
                dst.as_ref().display()
            );
        };
        let dst_parent: Cow<Path> = if dst_parent.as_os_str().is_empty() {
            Cow::Borrowed(dst_parent)
        } else {
            let dst_parent = self.temp_dir.canonicalize(dst_parent)?;
            self.temp_dir.create_dir_all(&dst_parent)?;
            Cow::Owned(dst_parent)
        };
        source_dir
            .dir_handle
            .hard_link(src, &self.temp_dir, dst_parent.join(dst_file_name))?;
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
            .dir_handle
            .open_with(src, cap_std::fs::OpenOptions::new().read(true))?;

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

    pub fn write_file<P>(&self, path: &P, data: &[u8]) -> io::Result<()>
    where
        P: AsRef<Path> + ?Sized,
    {
        let path = match self.temp_dir.canonicalize(path) {
            Ok(p) => RelPathBuf::new_checked(&p).map_err(io_err_map!(InvalidInput))?,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                util::normalize_path(path.as_ref())?
            }
            Err(err) => return Err(err),
        };

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
            at_root: path.as_os_str() == "." || path.as_os_str().is_empty(),
            inner: self.temp_dir.read_dir(&path)?,
        })
    }

    pub fn commit(self) -> io::Result<AtomicDir> {
        let (target_lock, prev_state) = match self.source {
            SourceDir::Existing(dir) => (dir.lock, Some((dir.dir_handle, dir.state))),
            SourceDir::New(lock) => (lock, None),
        };
        let mut target_lock = target_lock.upgrade()?;

        // We need to verify that the current state of the directory matches
        // what we expect. If not, we should abort the operation.
        let next_state = if let Some((existing_dir, curr_state)) = prev_state {
            let existing_state = read_state_file(&existing_dir)?;
            let LoadedDirState::Clean(existing_state) = existing_state else {
                io_bail!(
                    InvalidData,
                    "Directory has been poisoned due to previous failed operation. This is an inconsistent state: {}",
                    target_lock.path().display()
                );
            };
            if !existing_state.is_same(&curr_state) {
                io_bail!(
                    Other,
                    "Directory has changed since the update operation began. Current state: {:?}, expected state: {:?}. Path: {}",
                    existing_state,
                    curr_state,
                    target_lock.path().display()
                );
            }
            curr_state.to_next()
        } else {
            if target_lock
                .parent_dir()
                .try_exists(target_lock.file_name())?
            {
                io_bail!(
                    AlreadyExists,
                    "Target path already exists: {}",
                    target_lock.path().display()
                );
            }
            DirState::new()
        };
        {
            let mut state_file = self.temp_dir.open_with(
                STATE_FILE_NAME,
                cap_std::fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true),
            )?;
            state_file.write_all(&next_state.serialize()?)?;
            state_file.flush()?;
            state_file.sync_all()?;
        }
        let temp_path = self.temp_dir.dir_name().to_owned();
        let commit = CommitFileData::new(temp_path.clone(), create_old_path(&target_lock));

        // Release the temp dir so it doesn't get deleted.
        let _temp_dir = self.temp_dir.into_dir();

        if let Err(e) = commit.commit_file(&target_lock) {
            // Failed to write the commit file. Try to clean up after ourselves.
            drop(target_lock.parent_dir().remove_dir_all(temp_path));
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

    pub fn abort(self) -> io::Result<Option<AtomicDir>> {
        let prev_state = match self.source {
            SourceDir::Existing(dir) => Some(dir),
            SourceDir::New(_) => None,
        };
        Ok(prev_state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn create_atomic_dir() -> io::Result<()> {
        let dir = tempdir()?;

        let root = dir.path().join("testdir");
        let builder = DirBuilder::new_empty(&root)?;
        builder.write_file("file1.txt", b"Hello, world!")?;
        builder.commit()?;

        let contents = std::fs::read(root.join("file1.txt"))?;
        assert_eq!(&contents, b"Hello, world!");
        Ok(())
    }

    #[test]
    fn access_atomic_dir() -> io::Result<()> {
        let dir = tempdir()?;

        let root = dir.path().join("testdir");
        let builder = DirBuilder::new_empty(&root)?;
        builder.write_file("file1.txt", b"Hello, world!")?;
        let atomic_dir = builder.commit()?;

        assert_eq!(atomic_dir.read("file1.txt")?, b"Hello, world!");
        let entries = atomic_dir
            .read_dir(".")?
            .collect::<io::Result<Vec<_>>>()?
            .into_iter()
            .map(|e| e.file_name())
            .collect::<Vec<_>>();

        assert_eq!(entries, vec![OsString::from("file1.txt")]);
        Ok(())
    }

    #[test]
    fn update_atomic_dir() -> io::Result<()> {
        let dir = tempdir()?;

        let root = dir.path().join("testdir");
        let builder = DirBuilder::new_empty(&root)?;
        builder.write_file("file1.txt", b"Hello, world!")?;
        let atomic_dir = builder.commit()?;

        let updater = atomic_dir.begin_update()?;
        updater.write_file("file1.txt", b"Hello, universe!")?;
        let atomic_dir = updater.commit()?;

        assert_eq!(atomic_dir.read("file1.txt")?, b"Hello, universe!");
        Ok(())
    }

    #[test]
    fn update_copies_files() -> io::Result<()> {
        let dir = tempdir()?;

        let root = dir.path().join("testdir");
        let builder = DirBuilder::new_empty(&root)?;
        builder.write_file("file1.txt", b"Hello, world!")?;
        let atomic_dir = builder.commit()?;

        let updater = atomic_dir.begin_update()?;
        updater.copy_file("file1.txt", "file2.txt")?;
        updater.write_file("file1.txt", b"Hello, universe!")?;
        {
            let atomic_dir = updater.commit()?;
            assert_eq!(atomic_dir.read("file1.txt")?, b"Hello, universe!");
            assert_eq!(atomic_dir.read("file2.txt")?, b"Hello, world!");
        }

        let tempdir_entries = dir
            .path()
            .read_dir()?
            .collect::<io::Result<Vec<_>>>()?
            .into_iter()
            .map(|e| e.file_name())
            .collect::<Vec<_>>();

        assert_eq!(tempdir_entries, vec![OsString::from("testdir")]); // Only "testdir" should be present.
        Ok(())
    }

    #[test]
    fn aborted_update_cleans_up() -> io::Result<()> {
        let dir = tempdir()?;

        let root = dir.path().join("testdir");
        let builder = DirBuilder::new_empty(&root)?;
        builder.write_file("file1.txt", b"Hello, world!")?;
        let atomic_dir = builder.commit()?;

        let updater = atomic_dir.begin_update()?;
        updater.copy_file("file1.txt", "file2.txt")?;
        updater.write_file("file1.txt", b"Hello, universe!")?;
        {
            let atomic_dir = updater.abort()?.unwrap();
            assert_eq!(atomic_dir.read("file1.txt")?, b"Hello, world!");
        }

        let tempdir_entries = dir
            .path()
            .read_dir()?
            .collect::<io::Result<Vec<_>>>()?
            .into_iter()
            .map(|e| e.file_name())
            .collect::<Vec<_>>();

        assert_eq!(tempdir_entries, vec![OsString::from("testdir")]); // Only "testdir" should be present.
        Ok(())
    }
}
