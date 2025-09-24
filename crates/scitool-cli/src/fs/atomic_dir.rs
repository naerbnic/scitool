//! Atomic directory operations.

use std::{
    collections::BTreeMap,
    ffi::OsStr,
    io,
    path::{Component, Path, PathBuf},
};

use futures::StreamExt as _;
use rand::{Rng, distr::Alphanumeric};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncRead, AsyncReadExt as _, AsyncSeek, AsyncWrite, AsyncWriteExt as _},
    sync::Mutex,
};

use crate::fs::{
    err_helpers::{io_bail, io_err_map},
    ops::{
        FileSystemOperations, LockFile, OpenOptionsFlags, PathKind, TokioFileSystemOperations,
        WriteMode,
    },
    paths::{AbsPath, AbsPathBuf, RelPath, RelPathBuf},
};

const COMMIT_PATH: &str = ".DIR_COMMIT";
const LOCK_PATH: &str = ".DIR_LOCK";

async fn write_file_atomic<F, Fut, FS>(
    fs: &FS,
    base_dir: &AbsPath,
    tmp_dir: &AbsPath,
    write_mode: WriteMode,
    path: &RelPath,
    body: F,
) -> io::Result<()>
where
    F: FnOnce(FS::FileWriter) -> Fut,
    Fut: Future<Output = io::Result<()>>,
    FS: FileSystemOperations,
{
    let temp_path = tmp_dir.join_rel(path);
    let dest_path = base_dir.join_rel(path);
    // Create the parent directories for the given path.
    if let Some(parent) = temp_path.parent() {
        fs.create_dir_all(parent).await?;
    }

    fs.write_to_file(WriteMode::CreateNew, &tmp_dir.join(path), body)
        .await?;

    // Create the parent directories for the destination path, if needed.
    if let Some(parent) = dest_path.parent() {
        fs.create_dir_all(parent).await?;
    }

    match write_mode {
        // This will replace the destination file if it exists, but the change in the file data will
        // be atomic.
        WriteMode::Overwrite => fs.rename_file_atomic(&temp_path, &dest_path).await?,

        // This will do an atomic creation of the destination file, so only one attempt to
        // create the file will succeed.
        WriteMode::CreateNew => {
            fs.link_file_atomic(&temp_path, &dest_path).await?;
            // If the link succeeded, we can remove the temporary file.
            fs.remove_file(&temp_path).await?;
        }
    }

    Ok(())
}

/// An entry that indicates that a given file is located at either a temporary
/// path or its final destination.
///
/// This implies that if `temp_path` exists, it should be moved to `dest_path`
/// during a commit operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OverwriteEntry {
    /// The temporary path where the file is currently located. If this file
    /// exists, it is the authoritative source for the file's contents.
    temp_path: RelPathBuf,

    /// The final destination path where the file should be moved to during
    /// a commit operation.
    ///
    /// If the file at `temp_path` does not exist, it must exist here, and it
    /// is the authoritative source for the file's contents.
    dest_path: RelPathBuf,
}

impl OverwriteEntry {
    fn new_owned(temp_path: RelPathBuf, dest_path: RelPathBuf) -> OverwriteEntry {
        OverwriteEntry {
            temp_path,
            dest_path,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeleteEntry {
    /// The path to delete.
    path: RelPathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
enum CommitEntry {
    Overwrite(OverwriteEntry),
    Delete(DeleteEntry),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CommitSchema {
    temp_dir: RelPathBuf,
    /// The list of entries to commit.
    entries: Vec<CommitEntry>,
}

#[derive(Debug)]
pub struct FileRecoveryFailure {
    entry: CommitEntry,
    err: io::Error,
}

#[derive(Debug, thiserror::Error)]
pub struct RecoveryError {
    rename_failures: Vec<FileRecoveryFailure>,
}

impl std::fmt::Display for RecoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} out of {} file(s) failed to rename during recovery",
            self.rename_failures.len(),
            self.rename_failures.len() + self.rename_failures.len()
        )?;
        for failure in &self.rename_failures {
            match &failure.entry {
                CommitEntry::Delete(entry) => {
                    write!(
                        f,
                        "\nFailed to delete {}: {}",
                        entry.path.display(),
                        failure.err
                    )?;
                }
                CommitEntry::Overwrite(entry) => {
                    write!(
                        f,
                        "\nFailed to rename {} to {}: {}",
                        entry.temp_path.display(),
                        entry.dest_path.display(),
                        failure.err
                    )?;
                }
            }
        }
        Ok(())
    }
}

async fn apply_entry<FS: FileSystemOperations>(
    fs: &FS,
    dir_root: &Path,
    entry: CommitEntry,
    failed_entries: &mut Vec<FileRecoveryFailure>,
) -> io::Result<()> {
    match &entry {
        CommitEntry::Overwrite(overwrite_entry) => {
            let temp_path = dir_root.join(&*overwrite_entry.temp_path);
            let dest_path = dir_root.join(&*overwrite_entry.dest_path);
            match fs.rename_file_atomic(&temp_path, &dest_path).await {
                Ok(()) => { /* Successfully renamed */ }
                Err(err) if err.kind() == io::ErrorKind::NotFound => {
                    // Permitted: the temp file is missing, we assume the dest file is authoritative.
                }
                Err(err) => {
                    failed_entries.push(FileRecoveryFailure { entry, err });
                }
            }
        }
        CommitEntry::Delete(delete_entry) => {
            let path = dir_root.join(&*delete_entry.path);
            match fs.remove_file(&path).await {
                Err(err) if err.kind() == io::ErrorKind::NotFound => {
                    // Already deleted, nothing to do.
                }
                Ok(()) => { /* Successfully deleted */ }
                Err(err) => {
                    failed_entries.push(FileRecoveryFailure { entry, err });
                }
            }
        }
    }

    Ok(())
}

async fn recover_path<FS: FileSystemOperations>(fs: &FS, dir_root: &AbsPath) -> io::Result<()> {
    match fs.get_path_kind(dir_root).await? {
        PathKind::Directory => {
            // Normal case, nothing to do.
        }
        PathKind::File => io_bail!(
            NotADirectory,
            "Package root is a file: {}",
            dir_root.display()
        ),
        PathKind::Other => io_bail!(
            NotADirectory,
            "Package root is not a directory: {}",
            dir_root.display()
        ),
        PathKind::Missing => io_bail!(
            NotFound,
            "Package root does not exist: {}",
            dir_root.display()
        ),
    }

    let commit_path = dir_root.join_rel(RelPath::new_checked(COMMIT_PATH).unwrap());

    match fs.get_path_kind(&commit_path).await? {
        PathKind::File => {
            // We have some recovery to do. Continue.
        }
        PathKind::Directory => {
            io_bail!(InvalidData, "Commit file is a directory");
        }
        PathKind::Other => {
            io_bail!(InvalidData, "Commit file is not a regular file");
        }
        PathKind::Missing => {
            // No commit file, so no recovery needed.
            return Ok(());
        }
    }

    let commit_data_bytes = fs
        .read_file(&commit_path, |mut data| async move {
            let mut buf = Vec::new();
            data.read_to_end(&mut buf).await?;
            Ok(buf)
        })
        .await?;

    let mut commit_schema: CommitSchema = serde_json::from_slice(&commit_data_bytes)
        .map_err(io_err_map!(Other, "Failed to parse commit schema"))?;

    // Validate the paths in the commit schema.
    for entry in &commit_schema.entries {
        match entry {
            CommitEntry::Overwrite(entry) => {
                {
                    let path: &Path = &entry.temp_path;
                    is_valid_general_path(path)
                }?;
                {
                    let path: &Path = &entry.dest_path;
                    is_valid_general_path(path)
                }?;
            }
            CommitEntry::Delete(entry) => {
                {
                    let path: &Path = &entry.path;
                    is_valid_general_path(path)
                }?;
            }
        }
    }

    {
        let mut failed_entries = Vec::new();

        for entry in commit_schema.entries.drain(..) {
            apply_entry(fs, dir_root, entry, &mut failed_entries).await?;
        }

        if !failed_entries.is_empty() {
            io_bail!(
                Other,
                "File rename failures during recovery: {}",
                RecoveryError {
                    rename_failures: failed_entries
                }
            );
        }
    }

    // We have completed recovery, so we can remove the other files. First
    // delete all temporary directories.
    let mut root_entries = fs.list_dir(dir_root).await?;
    while let Some(entry_path) = root_entries
        .next()
        .await
        .transpose()
        .map_err(io_err_map!(Other, "Failed to read directory entry"))?
    {
        if entry_path
            .file_name()
            .and_then(OsStr::to_str)
            .is_some_and(|name| name.starts_with("tmpdir-"))
        {
            // This is a temporary directory, remove it.
            fs.remove_dir_all(&entry_path).await.ok();
        }
    }

    fs.remove_file(&commit_path).await?;

    Ok(())
}

async fn create_temp_dir<FS, R>(fs: &FS, rng: &mut R, base_dir: &Path) -> io::Result<RelPathBuf>
where
    FS: FileSystemOperations,
    R: Rng,
{
    for _ in 0..10 {
        let rand_str = rng
            .sample_iter(&Alphanumeric)
            .map(char::from)
            .take(16)
            .collect::<String>();

        let dir_name: PathBuf = format!("tmpdir-{rand_str}").into();
        let dir_name: RelPathBuf = dir_name.try_into().map_err(io_err_map!(
            Other,
            "Generated temporary directory name is not a valid relative path"
        ))?;
        let possible_temp_dir = base_dir.join(&dir_name);
        match fs.create_dir(&possible_temp_dir).await {
            Ok(()) => return Ok(dir_name),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                // Try again with a different name.
            }
            Err(err) => return Err(err),
        }
    }
    io_bail!(Other, "Failed to create a unique temporary directory");
}

fn is_valid_general_path(path: &Path) -> io::Result<&RelPath> {
    // The path must not have any components that are `..`, as this would allow
    // for directory traversal attacks.
    let path = RelPath::new_checked(path).map_err(io_err_map!(
        Other,
        "Path is not a valid relative path: {}",
        path.display()
    ))?;

    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => {
                io_bail!(
                    Other,
                    "Package file path must be strictly relative: {}",
                    path.display()
                );
            }
            Component::CurDir | Component::Normal(_) => {
                // We allow `.` components, as they are harmless.
            }
            Component::ParentDir => {
                io_bail!(
                    Other,
                    "Path must not contain a directory upreference: {}",
                    path.display()
                );
            }
        }
    }

    if path.components().any(|c| c == Component::ParentDir) {
        io_bail!(
            Other,
            "Path must not contain a directory upreference: {}",
            path.display()
        );
    }

    if path.starts_with(COMMIT_PATH) {
        io_bail!(
            Other,
            "Path must not start with the commit file name: {}",
            path.display()
        );
    }
    // The path cannot start with the commit file as a prefix, as this would
    // allow for accidental overwrites of in-progress commits.
    Ok(path)
}

fn normalize_path(path: &Path) -> io::Result<RelPathBuf> {
    let path = is_valid_general_path(path)?;
    let mut rel_path = RelPathBuf::new();

    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => {
                io_bail!(
                    Other,
                    "Package file path must be strictly relative: {}",
                    path.display()
                );
            }
            Component::ParentDir => {
                if !rel_path.pop() {
                    io_bail!(
                        Other,
                        "Path must not contain a directory upreference before the start: {}",
                        path.display()
                    );
                }
            }
            Component::CurDir => { /* Skip */ }
            Component::Normal(os_str) => {
                rel_path
                    .push(RelPath::new_checked(os_str).expect("Normal component is always valid"));
            }
        }
    }

    if rel_path.as_os_str().is_empty() {
        io_bail!(Other, "Destination path cannot be empty");
    }

    Ok(rel_path)
}

struct DirLock<LF>
where
    LF: LockFile + Send,
{
    _lock: LF,
}

impl<LF> DirLock<LF>
where
    LF: LockFile + Send,
{
    pub(crate) async fn acquire<FS>(fs: &FS, dir_root: &AbsPath) -> io::Result<Self>
    where
        FS: FileSystemOperations<FileLock = LF>,
    {
        let lock_path = dir_root.join_rel(RelPath::new_checked(LOCK_PATH).unwrap());
        let lock = fs.open_lock_file(&lock_path).await?;
        lock.lock_exclusive().await?;
        Ok(DirLock { _lock: lock })
    }
    pub(crate) async fn try_acquire<FS>(fs: &FS, dir_root: &AbsPath) -> io::Result<Option<Self>>
    where
        FS: FileSystemOperations<FileLock = LF>,
    {
        let lock_path = dir_root.join_rel(RelPath::new_checked(LOCK_PATH).unwrap());
        let lock = fs.open_lock_file(&lock_path).await?;
        if !lock.try_lock_exclusive().await? {
            return Ok(None);
        }
        Ok(Some(DirLock { _lock: lock }))
    }
}

enum TempFileStatus {
    /// The path is unchanged from the original file state.
    Unchanged,
    /// The path has been written to the temporary directory.
    Written,
    /// The path has been deleted from the final directory.
    Deleted,
}

struct AtomicDirState {
    file_statuses: BTreeMap<RelPathBuf, TempFileStatus>,
}

pub struct AtomicDirInner<FS: FileSystemOperations> {
    /// The file system operations implementation to use.
    fs: FS,

    /// A lock that ensures exclusive access to the directory.
    _dir_lock: DirLock<FS::FileLock>,

    /// The root directory being managed.
    dir_root: AbsPathBuf,

    /// The temporary directory inside the root directory.
    temp_dir: RelPathBuf,

    state: Mutex<AtomicDirState>,
}

impl<FS> AtomicDirInner<FS>
where
    FS: FileSystemOperations,
{
    fn relative_temp_file_path(&self, relative_path: &Path) -> io::Result<RelPathBuf> {
        let relative_path = is_valid_general_path(relative_path)?;
        Ok(self.temp_dir.join_rel(relative_path))
    }

    async fn create_at_dir_with_lock(
        fs: FS,
        dir_root: AbsPathBuf,
        dir_lock: DirLock<FS::FileLock>,
    ) -> io::Result<Self> {
        // It's possible that the previous operation was interrupted, so we
        // should try to recover the directory first.
        recover_path(&fs, &dir_root).await?;

        let temp_dir = create_temp_dir(&fs, &mut rand::rng(), &dir_root).await?;

        Ok(AtomicDirInner {
            fs,
            _dir_lock: dir_lock,
            dir_root,
            temp_dir,
            state: Mutex::new(AtomicDirState {
                file_statuses: BTreeMap::new(),
            }),
        })
    }

    pub async fn create_at_dir(fs: FS, dir_root: &Path) -> io::Result<Self> {
        let mut curr_dir = AbsPathBuf::new_checked(&std::env::current_dir()?)
            .map_err(io_err_map!(Other, "Failed to get current directory"))?;

        curr_dir.push(dir_root);
        let dir_root = curr_dir;
        let dir_lock = DirLock::acquire(&fs, &dir_root).await?;
        Self::create_at_dir_with_lock(fs, dir_root, dir_lock).await
    }

    pub async fn try_create_at_dir(fs: FS, dir_root: &Path) -> io::Result<Option<Self>> {
        let mut curr_dir = AbsPathBuf::new_checked(&std::env::current_dir()?)
            .map_err(io_err_map!(Other, "Failed to get current directory"))?;

        curr_dir.push(dir_root);
        let dir_root = curr_dir;
        let Some(dir_lock) = DirLock::try_acquire(&fs, &dir_root).await? else {
            return Ok(None);
        };
        Ok(Some(
            Self::create_at_dir_with_lock(fs, dir_root, dir_lock).await?,
        ))
    }

    pub async fn delete_path(&self, path: &Path) -> io::Result<()> {
        let rel_target_path = normalize_path(path)?;
        let rel_temp_path = self.relative_temp_file_path(&rel_target_path)?;
        let abs_target_path = self.dir_root.join_rel(&rel_target_path);
        let abs_temp_path = self.dir_root.join_rel(&rel_temp_path);

        let mut state_guard = self.state.lock().await;

        let file_status = state_guard
            .file_statuses
            .entry(rel_target_path.clone())
            .or_insert(TempFileStatus::Unchanged);

        match *file_status {
            TempFileStatus::Deleted => {
                // The file has already been deleted, no changes needed.
            }
            TempFileStatus::Unchanged => {
                match self.fs.get_path_kind(&abs_target_path).await? {
                    PathKind::Directory => io_bail!(IsADirectory, "Path is a directory"),
                    PathKind::Other => io_bail!(Other, "Path is not a regular file"),
                    PathKind::File => {}
                    PathKind::Missing => {
                        // The file does not exist. Avoid adding a delete entry
                        // to keep things clean.
                        return Ok(());
                    }
                }
            }
            TempFileStatus::Written => {
                // The file has been written to the temporary directory, so we
                // can just remove it from there.
                self.fs.remove_file(&abs_temp_path).await?;
            }
        }

        *file_status = TempFileStatus::Deleted;

        Ok(())
    }

    pub async fn write_at_path<F, Fut, R>(&self, dest_path: &Path, body: F) -> io::Result<R>
    where
        F: FnOnce(FS::File) -> Fut,
        Fut: Future<Output = io::Result<R>>,
    {
        let mut flags = OpenOptionsFlags::default();
        flags.set_create(true);
        flags.set_write(true);
        flags.set_truncate(true);
        let file = self.open_file(dest_path, &flags).await?;
        body(file).await
    }

    pub async fn open_file(&self, path: &Path, options: &OpenOptionsFlags) -> io::Result<FS::File> {
        let rel_target_path = normalize_path(path)?;
        let rel_target_parent = rel_target_path.parent_rel().unwrap_or_default();
        let abs_temp_root = self.dir_root.join_rel(&self.temp_dir);
        let abs_target_path = self.dir_root.join_rel(&rel_target_path);
        let abs_temp_path = abs_temp_root.join_rel(&rel_target_path);
        let abs_temp_parent = abs_temp_root.join_rel(rel_target_parent);

        let mut file_status_guard = self.state.lock().await;
        let file_status_guard = &mut *file_status_guard;
        let file_state_entry = file_status_guard
            .file_statuses
            .entry(rel_target_path.clone())
            .or_insert(TempFileStatus::Unchanged);
        match *file_state_entry {
            TempFileStatus::Written => {
                // The file has already been written to the temporary directory,
                // so we can open it directly from there.
                self.fs.open_file(&abs_temp_path, options).await
            }

            TempFileStatus::Deleted => {
                // The file has been deleted, so if we're creating it, we can
                // open it in the temporary directory. Otherwise, we should
                // return an error.
                if !options.can_create_file() {
                    io_bail!(
                        NotFound,
                        "File has been deleted: {}",
                        rel_target_path.display()
                    );
                }
                self.fs.create_dir_all(&abs_temp_parent).await?;

                let file = self.fs.open_file(&abs_temp_path, options).await?;
                *file_state_entry = TempFileStatus::Written;
                Ok(file)
            }

            TempFileStatus::Unchanged => {
                // We have not touched this file yet, so we need to set up its state.
                // We require that if there are any changes to a file, the data must be
                // only changed in the temp directory. This should be as transparent as
                // possible to the user.
                if !options.can_change_file() {
                    // We are not going to change the file, so we can open it directly.
                    return self.fs.open_file(&abs_target_path, options).await;
                }

                self.fs.create_dir_all(&abs_temp_parent).await?;

                {
                    let (should_create, should_copy) =
                        match self.fs.get_path_kind(&rel_target_path).await? {
                            PathKind::Directory => io_bail!(IsADirectory, "Path is a directory"),
                            PathKind::Other => io_bail!(Other, "Path is not a regular file"),
                            PathKind::File => (true, options.uses_original_data()),
                            PathKind::Missing => (false, false),
                        };

                    if should_create {
                        // Create an empty file in the temporary directory.
                        let mut target_flags = OpenOptionsFlags::default();
                        target_flags.set_write(true);
                        target_flags.set_create_new(true);
                        let mut target_file =
                            self.fs.open_file(&abs_temp_path, &target_flags).await?;
                        if should_copy {
                            // Copy the file to the temporary directory if we are going
                            // to change it.
                            let mut source_flags = OpenOptionsFlags::default();
                            source_flags.set_read(true);
                            let mut source_file =
                                self.fs.open_file(&abs_target_path, &source_flags).await?;

                            tokio::io::copy(&mut source_file, &mut target_file).await?;
                        }
                    }
                }

                let file = self.fs.open_file(&abs_temp_path, options).await?;
                *file_state_entry = TempFileStatus::Written;
                Ok(file)
            }
        }
    }

    pub async fn commit(self) -> io::Result<()> {
        let state = self.state.into_inner();
        let pending_commits = state
            .file_statuses
            .into_iter()
            .filter_map(|(path, status)| match status {
                TempFileStatus::Unchanged => None,
                TempFileStatus::Written => Some(CommitEntry::Overwrite(OverwriteEntry::new_owned(
                    self.temp_dir.join_rel(&path),
                    path,
                ))),
                TempFileStatus::Deleted => Some(CommitEntry::Delete(DeleteEntry { path })),
            })
            .collect::<Vec<_>>();
        if pending_commits.is_empty() {
            // Nothing to commit.
            return Ok(());
        }

        let commit_schema = CommitSchema {
            temp_dir: self.temp_dir.clone(),
            entries: pending_commits,
        };
        let commit_data = serde_json::to_vec(&commit_schema)
            .map_err(io_err_map!(Other, "Failed to serialize commit schema"))?;

        write_file_atomic(
            &self.fs,
            &self.dir_root,
            &self.dir_root.join_rel(&self.temp_dir),
            WriteMode::CreateNew,
            RelPath::new_checked(COMMIT_PATH).map_err(io_err_map!(
                Other,
                "Failed to create relative path for commit file"
            ))?,
            async |mut file| {
                file.write_all(&commit_data).await?;
                Ok(())
            },
        )
        .await?;

        // Now that we have written the commit file, we can perform recovery
        // to finalize the changes.
        recover_path(&self.fs, &self.dir_root).await?;

        Ok(())
    }
}

pub trait AtomicDirFile: AsyncRead + AsyncWrite + AsyncSeek + Send + Sync {
    fn close(self) -> impl Future<Output = io::Result<()>>;
}

impl AtomicDirFile for tokio::fs::File {
    async fn close(mut self) -> io::Result<()> {
        self.shutdown().await
    }
}

pub struct OpenOptions<'a> {
    parent: &'a AtomicDir,
    flags: OpenOptionsFlags,
}

impl OpenOptions<'_> {
    pub fn read(&mut self, read: bool) -> &mut Self {
        self.flags.set_read(read);
        self
    }

    pub fn write(&mut self, write: bool) -> &mut Self {
        self.flags.set_write(write);
        self
    }

    pub fn append(&mut self, append: bool) -> &mut Self {
        self.flags.set_append(append);
        self
    }

    pub fn truncate(&mut self, truncate: bool) -> &mut Self {
        self.flags.set_truncate(truncate);
        self
    }

    pub fn create(&mut self, create: bool) -> &mut Self {
        self.flags.set_create(create);
        self
    }

    pub fn create_new(&mut self, create_new: bool) -> &mut Self {
        self.flags.set_create_new(create_new);
        self
    }

    pub fn open<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl Future<Output = io::Result<impl AtomicDirFile + 'a>> + 'a {
        self.parent.open_file_impl(path, &self.flags)
    }
}

/// A high-level atomic directory that allows for changing files within
/// a directory atomically.
///
/// This guarantees that either all changes are applied to other `AtomicDir`
/// instances, or none are, even in the case of crashes or interruptions.
pub struct AtomicDir {
    inner: AtomicDirInner<TokioFileSystemOperations>,
}

impl AtomicDir {
    async fn open_file_impl(
        &self,
        path: &Path,
        options: &OpenOptionsFlags,
    ) -> io::Result<impl AtomicDirFile> {
        self.inner.open_file(path, options).await
    }

    pub async fn new_at_dir(dir_root: &Path) -> io::Result<Self> {
        let inner = AtomicDirInner::create_at_dir(TokioFileSystemOperations, dir_root).await?;
        Ok(AtomicDir { inner })
    }

    pub async fn try_new_at_dir(dir_root: &Path) -> io::Result<Option<Self>> {
        let Some(inner) =
            AtomicDirInner::try_create_at_dir(TokioFileSystemOperations, dir_root).await?
        else {
            return Ok(None);
        };
        Ok(Some(AtomicDir { inner }))
    }

    pub fn open_options(&self) -> OpenOptions<'_> {
        OpenOptions {
            parent: self,
            flags: OpenOptionsFlags::default(),
        }
    }

    pub fn delete<'a>(&'a self, path: &'a Path) -> impl Future<Output = io::Result<()>> + 'a {
        self.inner.delete_path(path)
    }

    pub fn commit(self) -> impl Future<Output = io::Result<()>> {
        self.inner.commit()
    }

    // Helper functions
    pub async fn write(&self, path: &Path, write_mode: WriteMode, data: &[u8]) -> io::Result<()> {
        let mut options = self.open_options();
        match write_mode {
            WriteMode::CreateNew => {
                options.create_new(true);
            }
            WriteMode::Overwrite => {
                options.create(true).truncate(true);
            }
        }
        let mut file = options.write(true).open(path).await?;
        file.write_all(data).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_write_and_commit() -> io::Result<()> {
        let dir = tempdir()?;

        let atomic_dir = AtomicDir::new_at_dir(dir.path()).await?;
        atomic_dir
            .write(Path::new("foo.txt"), WriteMode::CreateNew, b"hello")
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
    async fn test_recovery() -> io::Result<()> {
        let dir = tempdir()?;

        // Simulate a partial commit.
        let commit_schema = CommitSchema {
            temp_dir: RelPathBuf::new_checked("tmpdir-recovery-test").unwrap(),
            entries: vec![
                CommitEntry::Overwrite(OverwriteEntry::new_owned(
                    RelPathBuf::new_checked("tmpdir-recovery-test/foo.txt").unwrap(),
                    RelPathBuf::new_checked("foo.txt").unwrap(),
                )),
                CommitEntry::Delete(DeleteEntry {
                    path: RelPathBuf::new_checked("bar.txt").unwrap(),
                }),
            ],
        };

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
        } // _atomic_dir1 is dropped here, releasing the lock.

        // Now that the first lock is released, we should be able to acquire a new one.
        let atomic_dir3 = AtomicDir::try_new_at_dir(dir.path()).await?;
        assert!(
            atomic_dir3.is_some(),
            "Should be able to acquire lock after it's released"
        );

        Ok(())
    }
}
