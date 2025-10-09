//! Atomic directory implementation.

mod commit;
mod dir_lock;
mod dir_state;
mod recovery;
mod temp_dir;
mod types;
mod util;

use std::{
    borrow::Cow,
    collections::BTreeMap,
    ffi::OsString,
    io::{self, Read, Write},
    os::fd::{AsFd, AsRawFd},
    path::Path,
    sync::Mutex,
};

use cap_std::fs::Dir;

pub use self::types::FileType;
use crate::fs::{
    atomic_dir::{
        commit::CommitFileData,
        dir_lock::DirLock,
        dir_state::{DirState, LoadedDirState},
        recovery::{recover, recover_exclusive},
        temp_dir::{PersistError, TempDir},
        util::create_old_path,
    },
    err_helpers::{io_bail, io_err_map},
    file_lock::LockType,
    paths::{RelPath, RelPathBuf},
};

const STATE_FILE_NAME: &str = ".state";

/// The mode of creating a file in an atomic directory.
#[derive(Debug, Clone, Copy)]
pub enum CreateMode {
    /// Overwrite the file if it exists, or create it if it does not.
    Overwrite,
    /// Create the file if it does not exist. Fail if it already exists.
    CreateNew,
}

impl CreateMode {
    fn should_break_links(self) -> bool {
        match self {
            CreateMode::Overwrite => true,
            CreateMode::CreateNew => false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum OpenMode {
    /// Open the file for read-only access. Fails if the file does not exist.
    ReadOnly,
    /// Edit the existing file. Fails if file does not exist.
    Edit,
    /// Edit the file as is, or create an empty file if it does not exist.
    EditOrCreate,
    /// Edit an empty file
    EditEmpty(CreateMode),
}

impl OpenMode {
    fn should_break_links(self) -> bool {
        match self {
            OpenMode::Edit | OpenMode::EditOrCreate => true,
            OpenMode::ReadOnly => false,
            OpenMode::EditEmpty(create_mode) => create_mode.should_break_links(),
        }
    }

    fn can_create_file(self) -> bool {
        match self {
            OpenMode::Edit | OpenMode::ReadOnly => false,
            OpenMode::EditOrCreate | OpenMode::EditEmpty(_) => true,
        }
    }
}

/// The mode of an update operation on an atomic directory.
#[derive(Debug, Clone, Copy)]
pub enum UpdateInitMode {
    /// Copy all existing files into the temporary directory during
    /// initialization.
    CopyExisting,

    /// Start with an empty temporary directory.
    LeaveEmpty,
}

pub struct File {
    inner: cap_std::fs::File,
}

impl File {
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
    #[must_use]
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

impl Default for OpenOptions {
    fn default() -> Self {
        Self::new()
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
                    if !self.at_root || entry.file_name() != STATE_FILE_NAME {
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

impl Metadata {
    #[must_use]
    pub fn is_dir(&self) -> bool {
        self.inner.is_dir()
    }

    #[must_use]
    pub fn is_file(&self) -> bool {
        self.inner.is_file()
    }
}

/// Open and lock the atomic directory at the given path, recovering it if
/// necessary.
///
/// This still works if the directory does not yet exist, preventing other
/// processes from creating it while we are working.
fn lock_dir_safe(path: &Path, lock_type: LockType) -> io::Result<DirLock> {
    let mut target_lock = DirLock::acquire(path, lock_type)?;
    target_lock = recover(target_lock)?;

    Ok(target_lock)
}

fn try_open_dir(path: &Path) -> io::Result<Option<Dir>> {
    match cap_std::fs::Dir::open_ambient_dir(path, cap_std::ambient_authority()) {
        Ok(dir) => Ok(Some(dir)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

/// Reads the state file from the given directory, returning the loaded state.
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

fn link_all_files_recursively(
    source: &Dir,
    target: &Dir,
    relative_path: &Path,
    linked_files_sink: &mut Vec<RelPathBuf>,
) -> io::Result<()> {
    for entry in source.read_dir(relative_path)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let entry_path = relative_path.join(entry.file_name());
        let entry_path = source.canonicalize(&entry_path)?;

        if file_type.is_dir() {
            target.create_dir(&entry_path)?;
            link_all_files_recursively(source, target, &entry_path, linked_files_sink)?;
        } else if file_type.is_file() {
            source.hard_link(&entry_path, target, &entry_path)?;
            linked_files_sink.push(RelPathBuf::new_checked(&entry_path).expect("valid path"));
        } else {
            io_bail!(
                InvalidData,
                "Unsupported file type in atomic directory: {}",
                entry_path.display()
            );
        }
    }
    Ok(())
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
        Self::from_lock_and_handle(lock, dir_handle)
    }

    fn from_lock_and_handle(lock: DirLock, dir_handle: Dir) -> io::Result<Self> {
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
        Self::from_lock(lock_dir_safe(path.as_ref(), LockType::Shared)?)
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

    pub fn begin_update(self, init_mode: UpdateInitMode) -> io::Result<DirBuilder> {
        DirBuilder::from_atomic_dir(self, init_mode)
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

/// An error indicating that a commit operation failed.
/// 
/// This preserves the temporary directory so that the caller can attempt to recover
/// the changes later. If the caller does not need to recover, they can either
/// drop the error (which will delete the temporary directory), or propagate
/// it as an `std::io::Error` (which will also delete the temporary directory).
#[derive(Debug, thiserror::Error)]
#[error("Failed to commit changes: {source}")]
pub struct AbortError {
    #[source]
    source: io::Error,
    temp_dir: TempDir,
}

impl AbortError {
    /// Attempts to recover from the failed commit by moving the temporary
    /// directory to the target path.
    ///
    /// If this fails, the original error is returned, along with a new error
    /// indicating the failure to recover, and the temporary directory is
    /// returned so the caller can attempt again later.
    pub fn move_temp_dir_to<P>(self, path: &P) -> Result<io::Error, (io::Error, Self)>
    where
        P: AsRef<Path> + ?Sized,
    {
        let path = path.as_ref();
        let Self { source, temp_dir } = self;
        match temp_dir.persist_to(path) {
            Ok(()) => Ok(source),
            Err(persist_err) => {
                let PersistError {
                    cause: persist_err,
                    dir: temp_dir,
                } = persist_err;
                Err((persist_err, Self { source, temp_dir }))
            }
        }
    }
}

impl From<AbortError> for io::Error {
    fn from(err: AbortError) -> Self {
        // This drops the temporary directory, causing it to be deleted.
        err.source
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CommitError {
    /// The commit was aborted due to an error. The atomic directory was not
    /// modified, but the temporary directory may still exist and can be
    /// recovered from manually.
    #[error("Failed to commit changes: {0}")]
    Aborted(#[from] AbortError),

    /// The commit succeeded and is durable, but an error occured while trying
    /// to clean up the temporary directory. On a successful recovery, the
    /// atomic directory should be updated.
    #[error("Failed to recover from failed commit: {0}")]
    RecoveryFailed(#[source] io::Error),
}

impl From<CommitError> for io::Error {
    fn from(err: CommitError) -> Self {
        match err {
            CommitError::Aborted(e) => e.into(),
            CommitError::RecoveryFailed(e) => e,
        }
    }
}

/// A builder to create a new `AtomicDir`, or overwrite an existing one.
pub struct DirBuilder {
    source: SourceDir,
    temp_dir: TempDir,
    linked_files: Mutex<BTreeMap<RelPathBuf, RelPathBuf>>,
}

impl DirBuilder {
    fn from_source(source: SourceDir, init_mode: UpdateInitMode) -> io::Result<Self> {
        let lock = match &source {
            SourceDir::Existing(dir) => &dir.lock,
            SourceDir::New(lock) => lock,
        };
        let temp_dir = TempDir::new_in(lock.parent_dir().clone(), lock.file_name())?;

        let linked_files = if let UpdateInitMode::CopyExisting = init_mode
            && let SourceDir::Existing(dir) = &source
        {
            let mut linked_files = Vec::new();
            link_all_files_recursively(
                &dir.dir_handle,
                &temp_dir,
                Path::new("."),
                &mut linked_files,
            )?;
            linked_files
                .into_iter()
                .map(|path| (path.clone(), path))
                .collect()
        } else {
            BTreeMap::new()
        };

        Ok(DirBuilder {
            source,
            temp_dir,
            linked_files: Mutex::new(linked_files),
        })
    }

    fn source_dir(&self) -> io::Result<&Dir> {
        let SourceDir::Existing(dir) = &self.source else {
            io_bail!(Other, "No source directory available");
        };
        Ok(&dir.dir_handle)
    }

    fn copy_file_contents_from_source(&self, src: &Path, dst: &Path) -> io::Result<()> {
        self.source_dir()?.copy(src, &self.temp_dir, dst)?;
        Ok(())
    }

    fn canonicalize_in_temp(&self, path: &Path) -> io::Result<RelPathBuf> {
        match self.temp_dir.canonicalize(path) {
            Ok(p) => Ok(RelPathBuf::new_checked(&p).map_err(io_err_map!(InvalidInput))?),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(util::normalize_path(path)?),
            Err(err) => Err(err),
        }
    }

    fn ensure_parent(&self, path: &RelPath) -> io::Result<()> {
        let Some((dst_parent, _)) = util::safe_path_parent(path)? else {
            io_bail!(
                Other,
                "Destination path must have a parent path: {}",
                path.display()
            );
        };
        let path_parent: Cow<Path> = if dst_parent.as_os_str().is_empty() {
            Cow::Borrowed(dst_parent)
        } else {
            let dst_parent = self.temp_dir.canonicalize(dst_parent)?;
            self.temp_dir.create_dir_all(&dst_parent)?;
            Cow::Owned(dst_parent)
        };
        self.temp_dir.create_dir_all(path_parent)?;
        Ok(())
    }

    fn break_link(&self, path: &RelPath) -> io::Result<Option<RelPathBuf>> {
        let mut linked_files_guard = self.linked_files.lock().unwrap();
        if linked_files_guard.contains_key(path.as_path()) {
            // We need to unlink the existing hard link first, otherwise
            // we might end up modifying the original file.
            self.temp_dir.remove_file(path)?;
            Ok(linked_files_guard.remove(path.as_path()))
        } else {
            Ok(None)
        }
    }

    fn from_atomic_dir(dir: AtomicDir, init_mode: UpdateInitMode) -> io::Result<Self> {
        Self::from_source(SourceDir::Existing(dir), init_mode)
    }

    /// Creates a new atomic directory at the given path. Will fail if the path
    /// is not empty.
    pub fn new_at<P>(path: &P) -> io::Result<Self>
    where
        P: AsRef<Path> + ?Sized,
    {
        let path = path.as_ref();
        let target_lock = lock_dir_safe(path, LockType::Exclusive)?;
        if try_open_dir(path)?.is_some() {
            io_bail!(
                AlreadyExists,
                "Target path already exists: {}",
                target_lock.path().display()
            )
        }

        Self::from_source(SourceDir::New(target_lock), UpdateInitMode::LeaveEmpty)
    }

    /// Opens an existing atomic directory for update. Will fail if the
    /// directory does not exist.
    ///
    /// If `init_mode` is `CopyExisting`, all existing files from the source directory will be
    /// copied into the temporary directory. If `init_mode` is `LeaveEmpty`, the temporary directory
    /// will start out empty.
    pub fn open_existing_at(path: &Path, init_mode: UpdateInitMode) -> io::Result<Self> {
        let target_lock = lock_dir_safe(path, LockType::Shared)?;
        let root = try_open_dir(path)?.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "Target path does not exist: {}",
                    target_lock.path().display()
                ),
            )
        })?;

        let atomic_dir = AtomicDir::from_lock_and_handle(target_lock, root)?;

        Self::from_source(SourceDir::Existing(atomic_dir), init_mode)
    }

    /// Opens an atomic directory at the given path. If it does not yet exist, the builder is
    /// configured to create a new empty directory at that path on commit.
    ///
    /// If `init_mode` is `CopyExisting`, all existing files from the source directory will be
    /// copied into the temporary directory. If `init_mode` is `LeaveEmpty`, the temporary directory
    /// will start out empty.
    pub fn open_at(path: &Path, init_mode: UpdateInitMode) -> io::Result<Self> {
        // Take a shared lock, in case the directory already exists.
        let source_dir = loop {
            let target_lock = lock_dir_safe(path, LockType::Shared)?;
            if let Some(root) = try_open_dir(path)? {
                break SourceDir::Existing(AtomicDir::from_lock_and_handle(target_lock, root)?);
            }

            let target_lock = lock_dir_safe(path, LockType::Exclusive)?;
            if try_open_dir(path)?.is_none() {
                break SourceDir::New(target_lock);
            }
            // The directory was created after we took the shared lock, so try again.
        };

        Self::from_source(source_dir, init_mode)
    }

    /// Renames a file within the updated directory.
    ///
    /// Parent directories of the destination path will be created if they
    /// do not already exist.
    ///
    /// If the destination file already exists, it will be overwritten. If
    /// the destination is a directory, an error will be returned.
    pub fn rename_file<P1, P2>(&self, src: &P1, dst: &P2) -> io::Result<()>
    where
        P1: AsRef<Path> + ?Sized,
        P2: AsRef<Path> + ?Sized,
    {
        let src = self.canonicalize_in_temp(src.as_ref())?;
        let dst = self.canonicalize_in_temp(dst.as_ref())?;

        self.ensure_parent(&dst)?;
        self.temp_dir.rename(&src, &self.temp_dir, &dst)?;
        let mut linked_file_guard = self.linked_files.lock().unwrap();
        if let Some(source_path) = linked_file_guard.remove(src.as_path()) {
            linked_file_guard.insert(dst, source_path);
        }
        Ok(())
    }

    /// Copies a file within the updated directory.
    ///
    /// Parent directories of the destination path will be created if they
    /// do not already exist.
    ///
    /// If the destination file already exists, or if the destination is a directory,
    /// an error will be returned.
    pub fn copy_file<P1, P2>(&self, src: &P1, dst: &P2) -> io::Result<()>
    where
        P1: AsRef<Path> + ?Sized,
        P2: AsRef<Path> + ?Sized,
    {
        let src = self.canonicalize_in_temp(src.as_ref())?;
        let dst = self.canonicalize_in_temp(dst.as_ref())?;

        self.ensure_parent(&dst)?;
        let mut linked_file_guard = self.linked_files.lock().unwrap();
        if let Some(source_path) = linked_file_guard.get(src.as_path()) {
            self.source_dir()?
                .hard_link(source_path, &self.temp_dir, &dst)?;
            // Copy the link from the source file to the destination file.
            let source_path = source_path.clone();
            linked_file_guard.insert(dst, source_path);
        } else {
            drop(linked_file_guard);
            self.temp_dir.copy(&src, &self.temp_dir, &dst)?;
        }
        Ok(())
    }

    /// Opens a file within the updated directory.
    ///
    /// What mode the returned file is opened in depends on the `mode` parameter.
    ///
    /// - `OpenMode::ReadOnly`: Opens the file for read-only access. Fails if the file does not exist.
    /// - `OpenMode::Edit`: Opens the file for read and write access. Fails if the file does not exist.
    /// - `OpenMode::EditOrCreate`: Opens the file for read and write access, creating it if it does not exist. The file is otherwise left unchanged.
    /// - `OpenMode::EditEmpty(create_mode)`: Opens the file for write access, discarding any existing contents. The rules for creating the file depend on the `create_mode` parameter
    pub fn open_file(&self, path: &Path, mode: OpenMode) -> io::Result<File> {
        let path = self.canonicalize_in_temp(path)?;
        if mode.should_break_links()
            && let Some(source_path) = self.break_link(&path)?
        {
            // We had a hard link to the source file, so we need to copy
            // the contents from the source file to the temporary file.
            self.copy_file_contents_from_source(&source_path, &path)?;
        }

        if mode.can_create_file()
            && let Some(parent) = path.parent()
        {
            self.temp_dir.create_dir_all(parent)?;
        }

        let mut options = cap_std::fs::OpenOptions::new();
        match mode {
            OpenMode::ReadOnly => {
                options.read(true);
            }
            OpenMode::Edit => {
                options.read(true).write(true).create(false);
            }
            OpenMode::EditOrCreate => {
                options.read(true).write(true).create(true);
            }
            OpenMode::EditEmpty(create_mode) => {
                options.write(true);
                match create_mode {
                    CreateMode::Overwrite => {
                        options.create(true).truncate(true);
                    }
                    CreateMode::CreateNew => {
                        options.create_new(true);
                    }
                }
            }
        }
        let file = self.temp_dir.open_with(&path, &options)?;
        Ok(File { inner: file })
    }

    /// Writes the given data to a file within the updated directory.
    ///
    /// How the file is created depends on the `mode` parameter:
    /// - `CreateMode::Overwrite`: Overwrites the file if it exists, or creates it if it does not.
    /// - `CreateMode::CreateNew`: Creates the file if it does not exist. Fails if it already exists.
    pub fn write_file<P>(&self, path: &P, mode: CreateMode, data: &[u8]) -> io::Result<()>
    where
        P: AsRef<Path> + ?Sized,
    {
        let mut file = self.open_file(path.as_ref(), OpenMode::EditEmpty(mode))?;
        file.write_all(data)?;
        file.sync_all()?;
        Ok(())
    }

    /// Removes a file within the updated directory.
    pub fn remove_file<P>(&self, path: &P) -> io::Result<()>
    where
        P: AsRef<Path> + ?Sized,
    {
        let path = self.canonicalize_in_temp(path.as_ref())?;
        self.temp_dir.remove_file(&path)?;
        let mut linked_file_guard = self.linked_files.lock().unwrap();
        linked_file_guard.remove(&path);
        Ok(())
    }

    /// Reads the contents of a directory within the updated directory.
    ///
    /// The returned value is an iterator over the entries in the directory.
    pub fn read_dir(&self, path: &Path) -> io::Result<ReadDir> {
        let path = self.temp_dir.canonicalize(path)?;
        Ok(ReadDir {
            at_root: path.as_os_str() == "." || path.as_os_str().is_empty(),
            inner: self.temp_dir.read_dir(&path)?,
        })
    }

    /// Commits the changes made in the builder to the target directory.
    ///
    /// Committing is atomic and durable. It is atomic in the sense that if a
    /// shared lock is held for the directory, and an opportunity for recovery
    /// is given, either all changes will be visible, or none will. It is
    /// durable in the sense that once the commit operation has completed,
    /// the changes will be visible even in the event of a crash or power loss.
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
        let builder = DirBuilder::new_at(&root)?;
        builder.write_file("file1.txt", CreateMode::Overwrite, b"Hello, world!")?;
        builder.commit()?;

        let contents = std::fs::read(root.join("file1.txt"))?;
        assert_eq!(&contents, b"Hello, world!");
        Ok(())
    }

    #[test]
    fn access_atomic_dir() -> io::Result<()> {
        let dir = tempdir()?;

        let root = dir.path().join("testdir");
        let builder = DirBuilder::new_at(&root)?;
        builder.write_file("file1.txt", CreateMode::Overwrite, b"Hello, world!")?;
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
        let builder = DirBuilder::new_at(&root)?;
        builder.write_file("file1.txt", CreateMode::Overwrite, b"Hello, world!")?;
        let atomic_dir = builder.commit()?;

        let updater = atomic_dir.begin_update(UpdateInitMode::LeaveEmpty)?;
        updater.write_file("file1.txt", CreateMode::Overwrite, b"Hello, universe!")?;
        let atomic_dir = updater.commit()?;

        assert_eq!(atomic_dir.read("file1.txt")?, b"Hello, universe!");
        Ok(())
    }

    #[test]
    fn update_can_copy_files() -> io::Result<()> {
        let dir = tempdir()?;

        let root = dir.path().join("testdir");
        let builder = DirBuilder::new_at(&root)?;
        builder.write_file("file1.txt", CreateMode::Overwrite, b"Hello, world!")?;
        let atomic_dir = builder.commit()?;

        let updater = atomic_dir.begin_update(UpdateInitMode::CopyExisting)?;
        updater.rename_file("file1.txt", "file2.txt")?;
        updater.write_file("file1.txt", CreateMode::Overwrite, b"Hello, universe!")?;
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
        let builder = DirBuilder::new_at(&root)?;
        builder.write_file("file1.txt", CreateMode::Overwrite, b"Hello, world!")?;
        let atomic_dir = builder.commit()?;

        let updater = atomic_dir.begin_update(UpdateInitMode::CopyExisting)?;
        updater.rename_file("file1.txt", "file2.txt")?;
        updater.write_file("file1.txt", CreateMode::Overwrite, b"Hello, universe!")?;
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
