use std::{
    io,
    path::{Component, Path, PathBuf},
};

use rand::distr::SampleString as _;
use serde::{Deserialize, Serialize};

use crate::fs::{
    atomic_dir::{commit::CommitFileData, dir_lock::DirLock},
    err_helpers::io_bail,
    file_lock::LockType,
};

const COMMIT_FILE_SUFFIX: &str = ".commit";

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
    commit_file_path.try_exists()
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

    let commit_file_path = dir_lock.adjacent_ext_path(COMMIT_FILE_SUFFIX);
    let Some(mut commit) = CommitFileData::read_at(dir_lock)? else {
        // No commit file, so no recovery needed.
        return Ok(());
    };

    let temp_dir_path = dir_lock.parent().join(commit.temp_dir());
    let mut old_dir_path = dir_lock.parent().join(commit.old_dir());

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
                    commit = commit.with_old_dir(PathBuf::from(&new_old_dir_name));
                    commit.commit_file(dir_lock)?;
                    old_dir_path = dir_lock.parent().join(&commit.old_dir());
                }
            }
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

#[expect(dead_code, reason = "Primitive for current work")]
pub(crate) fn recover(dir_lock: &mut DirLock) -> io::Result<()> {
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
