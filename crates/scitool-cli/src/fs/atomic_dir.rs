//! Atomic directory operations.

use std::{
    ffi::OsStr,
    io,
    path::{Component, Path, PathBuf},
};

use futures::StreamExt as _;
use rand::{Rng, distr::Alphanumeric};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

use crate::fs::{
    err_helpers::{io_bail, io_err_map},
    ops::{FileSystemOperations, LockFile, PathKind, WriteMode},
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

fn is_valid_dest_path(path: &Path) -> io::Result<&RelPath> {
    is_valid_general_path(path)
}

fn is_valid_temp_path(path: &Path) -> io::Result<&RelPath> {
    is_valid_general_path(path)
}

fn normalize_dest_path(path: &Path) -> io::Result<RelPathBuf> {
    let path = is_valid_dest_path(path)?;
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

    let path: RelPathBuf = path.try_into().map_err(io_err_map!(
        Other,
        "Normalized destination path is not a valid relative path"
    ))?;

    Ok(path)
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

pub struct AtomicDirInner<FS: FileSystemOperations> {
    fs: FS,
    _dir_lock: DirLock<FS::FileLock>,
    dir_root: AbsPathBuf,
    temp_dir: RelPathBuf,
    pending_commits: Vec<CommitEntry>,
}

impl<FS> AtomicDirInner<FS>
where
    FS: FileSystemOperations,
{
    fn relative_temp_file_path(&self, relative_path: &Path) -> io::Result<RelPathBuf> {
        let relative_path = is_valid_dest_path(relative_path)?;
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
            pending_commits: Vec::new(),
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

    pub async fn delete_path(&mut self, relative_path: &Path) -> io::Result<()> {
        let relative_path = normalize_dest_path(relative_path)?;

        self.pending_commits.push(CommitEntry::Delete(DeleteEntry {
            path: relative_path,
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
            temp_dir: self.temp_dir.clone(),
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

#[cfg(test)]
mod tests {
    use crate::fs::ops::TokioFileSystemOperations;

    use super::*;
    use tempfile::tempdir;
    use tokio::io::AsyncReadExt;

    async fn read_file_to_string<P: AsRef<Path>>(
        fs: &TokioFileSystemOperations,
        path: P,
    ) -> io::Result<String> {
        fs.read_file(path.as_ref(), |mut file| async move {
            let mut contents = String::new();
            file.read_to_string(&mut contents).await?;
            Ok(contents)
        })
        .await
    }

    #[tokio::test]
    async fn test_write_and_commit() -> io::Result<()> {
        let dir = tempdir()?;
        let fs = TokioFileSystemOperations;

        let mut atomic_dir = AtomicDirInner::create_at_dir(fs, dir.path()).await?;
        atomic_dir
            .write_at_path(Path::new("foo.txt"), |mut file| async move {
                file.write_all(b"hello").await?;
                Ok(())
            })
            .await?;

        atomic_dir.commit().await?;

        let contents =
            read_file_to_string(&TokioFileSystemOperations, &dir.path().join("foo.txt")).await?;
        assert_eq!(contents, "hello");

        Ok(())
    }

    #[tokio::test]
    async fn test_delete_and_commit() -> io::Result<()> {
        let dir = tempdir()?;
        let fs = TokioFileSystemOperations;

        // Create a file to be deleted.
        tokio::fs::write(dir.path().join("foo.txt"), "hello").await?;

        let mut atomic_dir = AtomicDirInner::create_at_dir(fs, dir.path()).await?;
        atomic_dir.delete_path(Path::new("foo.txt")).await?;
        atomic_dir.commit().await?;

        assert!(!dir.path().join("foo.txt").exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_write_delete_and_commit() -> io::Result<()> {
        let dir = tempdir()?;
        let fs = TokioFileSystemOperations;

        // Create a file to be overwritten.
        tokio::fs::write(dir.path().join("foo.txt"), "old content").await?;

        let mut atomic_dir = AtomicDirInner::create_at_dir(fs, dir.path()).await?;
        atomic_dir
            .write_at_path(Path::new("foo.txt"), |mut file| async move {
                file.write_all(b"new content").await?;
                Ok(())
            })
            .await?;
        atomic_dir.delete_path(Path::new("bar.txt")).await?;
        atomic_dir
            .write_at_path(Path::new("bar.txt"), |mut file| async move {
                file.write_all(b"new file").await?;
                Ok(())
            })
            .await?;
        atomic_dir.commit().await?;

        let contents =
            read_file_to_string(&TokioFileSystemOperations, &dir.path().join("foo.txt")).await?;
        assert_eq!(contents, "new content");
        let contents_bar =
            read_file_to_string(&TokioFileSystemOperations, &dir.path().join("bar.txt")).await?;
        assert_eq!(contents_bar, "new file");

        Ok(())
    }

    #[tokio::test]
    async fn test_recovery() -> io::Result<()> {
        let dir = tempdir()?;
        let fs = TokioFileSystemOperations;

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
        let _atomic_dir = AtomicDirInner::create_at_dir(fs, dir.path()).await?;

        // Check that recovery happened.
        let contents =
            read_file_to_string(&TokioFileSystemOperations, &dir.path().join("foo.txt")).await?;
        assert_eq!(contents, "recovered");
        assert!(!dir.path().join("bar.txt").exists());
        assert!(!dir.path().join(COMMIT_PATH).exists());
        assert!(!dir.path().join("tmpdir-recovery-test").exists());

        Ok(())
    }
}
