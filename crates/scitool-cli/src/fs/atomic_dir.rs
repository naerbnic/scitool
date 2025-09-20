//! Atomic directory operations.

use std::{
    borrow::Cow,
    ffi::OsStr,
    io,
    path::{Component, Path, PathBuf},
};

use futures::{Stream, StreamExt as _};
use rand::{Rng, distr::Alphanumeric};
use serde::{Deserialize, Serialize};
use tokio::{
    fs::File,
    io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _},
};

use crate::fs::{
    err_helpers::{io_bail, io_err, io_err_map},
    owned_arc::{MutBorrowedArc, loan_arc},
    paths::{AbsPath, AbsPathBuf, RelPath, RelPathBuf, classify_path},
};

const COMMIT_PATH: &str = ".DIR_COMMIT";

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

/// A trait for the file system operations needed by `AtomicDir`.
pub trait FileSystemOperations {
    type FileReader: AsyncRead + Unpin + Send;
    type FileWriter: AsyncWrite + Unpin + Send;

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
}

pub struct TokioFileSystemOperations;

impl FileSystemOperations for TokioFileSystemOperations {
    type FileReader = MutBorrowedArc<File>;
    type FileWriter = File;

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
}

#[derive(Debug)]
struct SharedPathItem<'a, T>(Cow<'a, T>)
where
    T: ToOwned + ?Sized,
    T::Owned: std::fmt::Debug;

impl<T> Clone for SharedPathItem<'_, T>
where
    T: ToOwned + ?Sized,
    T::Owned: std::fmt::Debug,
{
    fn clone(&self) -> Self {
        SharedPathItem(self.0.clone())
    }
}

impl<'a, T> SharedPathItem<'a, T>
where
    T: ToOwned + ?Sized,
    T::Owned: std::fmt::Debug,
{
    fn new<P>(path: P) -> Self
    where
        P: Into<Cow<'a, T>>,
    {
        SharedPathItem(path.into())
    }

    fn into_owned(self) -> SharedPathItem<'static, T> {
        SharedPathItem(Cow::Owned(self.0.into_owned()))
    }
}

impl<T> std::fmt::Display for SharedPathItem<'_, T>
where
    T: ToOwned + std::fmt::Display + ?Sized,
    T::Owned: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.as_ref().fmt(f)
    }
}

impl<T> Serialize for SharedPathItem<'_, T>
where
    T: ToOwned + Serialize + ?Sized,
    T::Owned: std::fmt::Debug,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        Serialize::serialize(&self.0, serializer)
    }
}

impl<'de: 'a, 'a, T> Deserialize<'de> for SharedPathItem<'a, T>
where
    T: ToOwned + ?Sized + 'de,
    T::Owned: std::fmt::Debug,
    &'de T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value: &'de T = Deserialize::deserialize(deserializer)?;
        Ok(SharedPathItem(Cow::Borrowed(value)))
    }
}

impl<T> std::ops::Deref for SharedPathItem<'_, T>
where
    T: ToOwned + ?Sized,
    T::Owned: std::fmt::Debug,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

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
    if let Some(parent) = dest_path.parent() {
        fs.create_dir_all(parent).await?;
    }

    fs.write_to_file(WriteMode::CreateNew, &tmp_dir.join(path), body)
        .await?;

    match write_mode {
        WriteMode::Overwrite => fs.rename_file_atomic(&temp_path, &dest_path).await?,
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
struct OverwriteEntry<'a> {
    /// The temporary path where the file is currently located. If this file
    /// exists, it is the authoritative source for the file's contents.
    #[serde(borrow)]
    temp_path: SharedPathItem<'a, Path>,

    /// The final destination path where the file should be moved to during
    /// a commit operation.
    ///
    /// If the file at `temp_path` does not exist, it must exist here, and it
    /// is the authoritative source for the file's contents.
    #[serde(borrow)]
    dest_path: SharedPathItem<'a, Path>,
}

impl OverwriteEntry<'_> {
    fn new_owned(temp_path: PathBuf, dest_path: PathBuf) -> OverwriteEntry<'static> {
        OverwriteEntry {
            temp_path: SharedPathItem(Cow::Owned(temp_path)),
            dest_path: SharedPathItem(Cow::Owned(dest_path)),
        }
    }

    fn into_owned(self) -> OverwriteEntry<'static> {
        OverwriteEntry {
            temp_path: self.temp_path.into_owned(),
            dest_path: self.dest_path.into_owned(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeleteEntry<'a> {
    /// The path to delete.
    #[serde(borrow)]
    path: SharedPathItem<'a, Path>,
}

impl DeleteEntry<'_> {
    fn into_owned(self) -> DeleteEntry<'static> {
        DeleteEntry {
            path: self.path.into_owned(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
enum CommitEntry<'a> {
    Overwrite(#[serde(borrow)] OverwriteEntry<'a>),
    Delete(#[serde(borrow)] DeleteEntry<'a>),
}

impl CommitEntry<'_> {
    fn into_owned(self) -> CommitEntry<'static> {
        match self {
            CommitEntry::Overwrite(entry) => CommitEntry::Overwrite(entry.into_owned()),
            CommitEntry::Delete(entry) => CommitEntry::Delete(entry.into_owned()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CommitSchema<'a> {
    temp_dir: SharedPathItem<'a, Path>,
    /// The list of entries to commit.
    #[serde(borrow)]
    entries: Vec<CommitEntry<'a>>,
}

#[derive(Debug)]
pub struct FileRecoveryFailure {
    entry: CommitEntry<'static>,
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
    entry: CommitEntry<'_>,
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
                    failed_entries.push(FileRecoveryFailure {
                        entry: entry.into_owned(),
                        err,
                    });
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
                    failed_entries.push(FileRecoveryFailure {
                        entry: entry.into_owned(),
                        err,
                    });
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
                is_valid_temp_path(&entry.temp_path)?;
                is_valid_dest_path(&entry.dest_path)?;
            }
            CommitEntry::Delete(entry) => {
                is_valid_dest_path(&entry.path)?;
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
    while root_entries
        .next()
        .await
        .transpose()
        .map_err(io_err_map!(Other, "Failed to read directory entry"))?
        .is_some()
    {
        let entry_path = root_entries
            .next()
            .await
            .transpose()
            .map_err(io_err_map!(Other, "Failed to read directory entry"))?;
        if let Some(entry_path) = entry_path
            && entry_path
                .file_name()
                .and_then(OsStr::to_str)
                .is_some_and(|name| name.starts_with("tmpdir-") || name.starts_with("tmpdir-"))
        {
            // This is a temporary directory, remove it.
            fs.remove_dir(&entry_path).await.ok();
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

fn is_valid_general_path(path: &Path) -> io::Result<()> {
    // The path must not have any components that are `..`, as this would allow
    // for directory traversal attacks.
    if path.is_absolute() {
        io_bail!(
            Other,
            "Package file path must not be absolute: {}",
            path.display()
        );
    }

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
    Ok(())
}

fn is_valid_dest_path(path: &Path) -> io::Result<()> {
    is_valid_general_path(path)
}

fn is_valid_temp_path(path: &Path) -> io::Result<()> {
    is_valid_general_path(path)
}

fn normalize_dest_path(path: &Path) -> io::Result<PathBuf> {
    is_valid_dest_path(path)?;
    let path: PathBuf = path
        .components()
        .filter_map(|c| match c {
            Component::Prefix(_) | Component::RootDir | Component::ParentDir => {
                panic!("Invalid path component in destination path")
            }
            Component::CurDir => None,
            Component::Normal(os_str) => Some(os_str),
        })
        .collect();

    if path.as_os_str().is_empty() {
        io_bail!(Other, "Destination path cannot be empty");
    }

    Ok(path)
}

pub struct AtomicDirInner<FS: FileSystemOperations> {
    fs: FS,
    dir_root: AbsPathBuf,
    temp_dir: RelPathBuf,
    pending_commits: Vec<CommitEntry<'static>>,
}

impl<FS> AtomicDirInner<FS>
where
    FS: FileSystemOperations,
{
    fn relative_temp_file_path(&self, relative_path: &Path) -> io::Result<PathBuf> {
        is_valid_dest_path(relative_path)?;
        Ok(self.temp_dir.join(relative_path))
    }

    pub async fn create_at_dir(fs: FS, dir_root: &Path) -> io::Result<Self> {
        let dir_root = classify_path(dir_root)
            .map_err(io_err_map!(Other, "Failed to classify directory root path"))?
            .as_abs()
            .ok_or_else(|| {
                io_err!(
                    Other,
                    "Directory root path must be absolute: {}",
                    dir_root.display()
                )
            })?;
        // It's possible that the previous operation was interrupted, so we
        // should try to recover the directory first.
        recover_path(&fs, dir_root).await?;

        let temp_dir = create_temp_dir(&fs, &mut rand::rng(), dir_root).await?;

        Ok(AtomicDirInner {
            fs,
            dir_root: dir_root.to_abs_path_buf(),
            temp_dir,
            pending_commits: Vec::new(),
        })
    }

    pub async fn delete_path(&mut self, relative_path: &Path) -> io::Result<()> {
        let relative_path = normalize_dest_path(relative_path)?;

        self.pending_commits.push(CommitEntry::Delete(DeleteEntry {
            path: SharedPathItem(relative_path.into()),
        }));

        Ok(())
    }

    pub async fn write_at_path<F, Fut, R>(&mut self, dest_path: &Path, body: F) -> io::Result<R>
    where
        F: FnOnce(FS::FileWriter) -> Fut,
        Fut: Future<Output = io::Result<R>>,
    {
        let dest_path = normalize_dest_path(dest_path)?;
        let rel_temp_path = self.relative_temp_file_path(&dest_path)?;
        let result = self
            .fs
            .write_to_file(
                WriteMode::Overwrite,
                &self.dir_root.join(&rel_temp_path),
                body,
            )
            .await?;

        self.pending_commits
            .push(CommitEntry::Overwrite(OverwriteEntry::new_owned(
                rel_temp_path,
                dest_path,
            )));

        Ok(result)
    }

    pub async fn commit(mut self) -> io::Result<()> {
        if self.pending_commits.is_empty() {
            // Nothing to commit.
            return Ok(());
        }

        let commit_schema = CommitSchema {
            temp_dir: SharedPathItem::<Path>::new(self.temp_dir.as_path()).into_owned(),
            entries: self.pending_commits.drain(..).collect(),
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
