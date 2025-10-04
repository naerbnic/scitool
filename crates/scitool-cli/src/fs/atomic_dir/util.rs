use std::{
    fs::TryLockError,
    io::{self, Write as _},
    path::{Component, Path, PathBuf},
};

use cap_std::fs::Dir;
use rand::distr::SampleString as _;
use serde::{Deserialize, Serialize};
use tempfile::env::temp_dir;

const LOCK_FILE_SUFFIX: &str = ".lock";
const COMMIT_FILE_SUFFIX: &str = ".commit";

use crate::fs::{
    atomic_dir::{COMMIT_PATH, LOCK_PATH},
    err_helpers::{io_bail, io_err, io_err_map},
    file_lock::{
        LockType,
        ephemeral::{self, EphemeralFileLock},
    },
    ops::{FileSystemOperations, WriteMode},
    paths::{AbsPath, RelPath},
};

pub(super) fn is_valid_path<'a>(path: &'a Path, temp_dir: &Path) -> io::Result<&'a RelPath> {
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

    // The path cannot start with the commit file or lock file as a prefix, as
    // this would allow for accidental overwrites of in-progress commits.
    if path.starts_with(COMMIT_PATH) {
        io_bail!(
            Other,
            "Path must not start with the commit file name: {}",
            path.display()
        );
    }

    if path.starts_with(LOCK_PATH) {
        io_bail!(
            Other,
            "Path must not start with the lock file name: {}",
            path.display()
        );
    }

    if path.starts_with(temp_dir) {
        io_bail!(
            Other,
            "Path must not start with the temporary directory name: {}",
            path.display()
        );
    }
    Ok(path)
}

pub(super) fn write_file_atomic2(
    path: &Path,
    data: &[u8],
    write_mode: WriteMode,
) -> io::Result<()> {
    let Some(parent) = path.parent() else {
        io_bail!(
            Other,
            "Path must have a parent directory: {}",
            path.display()
        )
    };

    std::fs::create_dir_all(parent)?;

    let mut temp_file = tempfile::Builder::new()
        .suffix(".tmp")
        .tempfile_in(parent)?;
    {
        let temp_file = temp_file.as_file_mut();
        temp_file.write_all(data)?;
        temp_file.flush()?;
        temp_file.sync_data()?;
    }

    let file = match write_mode {
        // This will replace the destination file if it exists, but the change in the file data will
        // be atomic.
        //
        // Note that other file handles with this file open will not see the new data until they
        // reopen the file. If data wants to be persisted
        WriteMode::Overwrite => temp_file.persist(path)?,

        // This will do an atomic creation of the destination file, so only one attempt to
        // create the file will succeed.
        //
        // The file data will appear atomic, but it's possible that the temp file could be left
        // in a crash scenario, even if the move to the final location succeeded.
        WriteMode::CreateNew => temp_file.persist_noclobber(path)?,
    };

    drop(file);

    Ok(())
}

fn extract_singleton<I: IntoIterator>(iter: I) -> io::Result<I::Item> {
    let mut iter = iter.into_iter();
    let Some(first) = iter.next() else {
        io_bail!(InvalidData, "Expected exactly one item, found none");
    };
    if iter.next().is_some() {
        io_bail!(InvalidData, "Expected exactly one item, found multiple");
    }
    Ok(first)
}

pub(super) struct DirLock {
    target_path: PathBuf,
    lock_type: LockType,
    lock_file: Option<EphemeralFileLock>,
}

impl DirLock {
    pub(super) fn path(&self) -> &Path {
        &self.target_path
    }

    pub(super) fn file_name(&self) -> &std::ffi::OsStr {
        self.target_path
            .file_name()
            .expect("DirLock target path validated in constructors")
    }

    pub(super) fn parent(&self) -> &Path {
        self.target_path
            .parent()
            .expect("DirLock target path validated in constructors")
    }

    pub(super) fn acquire(path: &Path, lock_type: LockType) -> io::Result<Self> {
        let Some(file_name) = path.file_name() else {
            io_bail!(Other, "Path must have a file name: {}", path.display());
        };
        let lock_file_path =
            path.with_file_name(format!("{}{}", file_name.display(), LOCK_FILE_SUFFIX));
        let lock_file = ephemeral::lock_file(&lock_file_path, lock_type)?;
        Ok(Self {
            target_path: path.to_path_buf(),
            lock_type,
            lock_file: Some(lock_file),
        })
    }

    pub(super) fn try_acquire(path: &Path, lock_type: LockType) -> Result<Self, TryLockError> {
        let Some(file_name) = path.file_name() else {
            return Err(TryLockError::Error(io_err!(
                Other,
                "Path must have a file name: {}",
                path.display()
            )));
        };
        let lock_file_path =
            path.with_file_name(format!("{}{}", file_name.display(), LOCK_FILE_SUFFIX));
        let lock_file = ephemeral::try_lock_file(&lock_file_path, lock_type)?;
        Ok(Self {
            target_path: path.to_path_buf(),
            lock_type,
            lock_file: Some(lock_file),
        })
    }

    pub(super) fn lock_type(&self) -> LockType {
        self.lock_type
    }

    pub(super) fn adjacent_ext_path(&self, ext: &str) -> PathBuf {
        let file_name = self
            .target_path
            .file_name()
            .expect("DirLock target path must have a file name");
        self.target_path
            .with_file_name(format!("{}{}", file_name.display(), ext))
    }

    pub(super) fn upgrade(&mut self) -> io::Result<()> {
        let Some(lock_file) = self.lock_file.as_mut() else {
            io_bail!(Other, "Lock has already been released");
        };
        lock_file.upgrade()?;
        Ok(())
    }

    pub(super) fn downgrade(&mut self) -> io::Result<()> {
        let Some(lock_file) = self.lock_file.as_mut() else {
            io_bail!(Other, "Lock has already been released");
        };
        lock_file.downgrade()?;
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct CommitContents {
    /// The version of the commit schema, in case it changes.
    version: u32,

    /// The location of the temp file that was being moved during the commit.
    ///
    /// Must be a directory only, i.e. a relative path with a single normal component.
    temp_dir: PathBuf,

    /// The location the old directory will be moved to.
    ///
    /// Must be a directory only, i.e. a relative path with a single normal component.
    old_dir: PathBuf,
}

impl CommitContents {
    fn validate(&self) -> io::Result<()> {
        if self.version != 1 {
            io_bail!(
                InvalidData,
                "Unsupported commit schema version: {}",
                self.version
            );
        }

        if let Component::Normal(_) = extract_singleton(self.temp_dir.components())? {
            io_bail!(
                InvalidData,
                "Temp dir must be a single normal path component: {}",
                self.temp_dir.display()
            );
        }

        if let Component::Normal(_) = extract_singleton(self.old_dir.components())? {
            io_bail!(
                InvalidData,
                "Old dir must be a single normal path component: {}",
                self.old_dir.display()
            );
        }

        Ok(())
    }
}

pub(crate) fn check_needs_recovery(dir_lock: &DirLock) -> io::Result<bool> {
    let commit_file_path = dir_lock.adjacent_ext_path(COMMIT_FILE_SUFFIX);
    Ok(commit_file_path.try_exists()?)
}

enum TryRenameResult {
    Success,
    SourceMissing,
    TargetExists,
}

fn try_rename(src: &Path, dst: &Path) -> io::Result<TryRenameResult> {
    match std::fs::rename(src, dst) {
        Ok(()) => Ok(TryRenameResult::Success),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(TryRenameResult::SourceMissing),
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => Ok(TryRenameResult::TargetExists),
        Err(err) => Err(err),
    }
}

pub(crate) fn recover_exclusive(dir_lock: &DirLock) -> io::Result<()> {
    if dir_lock.lock_type() != LockType::Exclusive {
        io_bail!(Other, "DirLock must be exclusive to recover");
    }

    let mut commit_options = std::fs::OpenOptions::new();
    let commit_options = commit_options
        .create(false)
        .read(true)
        .write(true)
        .truncate(false);

    let commit_file_path = dir_lock.adjacent_ext_path(COMMIT_FILE_SUFFIX);
    let commit_file = match commit_options.open(&commit_file_path) {
        Ok(file) => file,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            // No commit file, so no recovery needed.
            return Ok(());
        }
        Err(err) => {
            return Err(err);
        }
    };

    let mut commit: CommitContents = serde_json::from_reader(&commit_file).map_err(io_err_map!(
        InvalidData,
        "Failed to parse commit file: {}",
        commit_file_path.display()
    ))?;
    commit.validate()?;

    let temp_dir_path = dir_lock.parent().join(&commit.temp_dir);
    let mut old_dir_path = dir_lock.parent().join(&commit.old_dir);

    let mut target_exists = dir_lock.path().try_exists()?;
    let mut temp_exists = temp_dir_path.try_exists()?;
    let mut old_exists = old_dir_path.try_exists()?;

    // We go through each step of the recovery process, checking the state of
    // each of the three directories (target, temp, old) to determine what
    // action to take.

    // Sanity check: We should always have either the target or the temp
    // directory existing, otherwise there's nothing to recover.
    if !target_exists && !temp_exists {
        io_bail!(
            InvalidData,
            "Corrupted state: Neither target nor temp directory exists!"
        );
    }

    // Step 1: Move the target to old.
    if temp_exists && target_exists {
        // Move the target to old, then move temp to target.
        loop {
            match try_rename(dir_lock.path(), &old_dir_path)? {
                TryRenameResult::Success => {
                    break;
                }
                TryRenameResult::SourceMissing => {
                    // Err, this shouldn't happen, since we have the lock.
                    io_bail!(
                        NotFound,
                        "Target directory disappeared during recovery: {}",
                        dir_lock.path().display()
                    );
                }
                TryRenameResult::TargetExists => {
                    // It looks like whatever the old_path already exists. We need to
                    // choose another one. We have to persist it to the commit file
                    // so that if we crash again after this step, we know where it is.
                    let suffix = rand::distr::Alphanumeric.sample_string(&mut rand::rng(), 6);
                    let new_old_dir_name =
                        format!("{}.old-{}", dir_lock.file_name().display(), suffix);
                    commit.old_dir = PathBuf::from(&new_old_dir_name);
                    let commit_data = serde_json::to_vec(&commit).map_err(io_err_map!(
                        Other,
                        "Failed to serialize updated commit data"
                    ))?;
                    write_file_atomic2(&commit_file_path, &commit_data, WriteMode::Overwrite)?;
                    old_dir_path = dir_lock.parent().join(&commit.old_dir);
                }
            };
        }

        target_exists = false;
        old_exists = true;
    }

    // Step 2: Move temp to target.
    if temp_exists && !target_exists {
        // Move temp to target.
        std::fs::rename(&temp_dir_path, dir_lock.path())?;
        temp_exists = false;
        target_exists = true;
    }

    // Step 3: Remove old.
    if !temp_exists && target_exists && old_exists {
        // We are done, just need to remove old.
        std::fs::remove_dir_all(&old_dir_path)?;
    }

    // Now, we should only have the target directory, and the recovery should be
    // complete. Delete the commit file to indicate that recovery is complete.
    std::fs::remove_file(commit_file_path)?;

    Ok(())
}

pub(crate) fn recover2(dir_lock: &mut DirLock) -> io::Result<()> {
    match dir_lock.lock_type() {
        LockType::Shared => {
            // Since we can't guarantee that when we upgrade/downgrade the lock
            // that another process won't sneak in and do the recovery first, we
            // need to loop until we either
            loop {
                if !check_needs_recovery(dir_lock)? {
                    return Ok(());
                }

                // We need to recover, but we only have a shared lock. We need to
                // upgrade to an exclusive lock.
                dir_lock.upgrade()?;

                recover_exclusive(dir_lock)?;

                dir_lock.downgrade()?;

                // Since we may release the lock to downgrade, we could have a
                // new recovery situation, so loop back to check again.
            }
        }
        LockType::Exclusive => {
            recover_exclusive(dir_lock)?;
        }
    }

    Ok(())
}
