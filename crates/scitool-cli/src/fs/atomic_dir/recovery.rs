use std::{
    ffi::OsStr,
    io::{self, Read as _},
};

use crate::fs::{
    err_helpers::{io_bail, io_err_map},
    io_wrappers::LengthLimitedReader,
    ops::{FileSystemOperations, PathKind},
    paths::{AbsPath, RelPath},
};

use super::{
    COMMIT_PATH,
    schema::{CURR_COMMIT_VERSION, CommitEntry, CommitSchema},
    util::is_valid_path,
};

/// Records the failure to recover a single file during a commit recovery process.
///
/// This struct is created when a file operation (like renaming or deleting) fails
/// as part of applying the changes from a commit log.
#[derive(Debug)]
pub(super) struct FileRecoveryFailure {
    /// The specific commit entry that could not be applied.
    entry: CommitEntry,
    /// The underlying I/O error that caused the failure.
    err: io::Error,
}

/// An error that occurs during the recovery of a partially committed atomic directory.
///
/// This error aggregates all individual file recovery failures that occurred during
/// an attempt to apply a commit log.
#[derive(Debug, thiserror::Error)]
pub(super) struct RecoveryError {
    /// A list of files that failed to be recovered.
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
                        entry.path().display(),
                        failure.err
                    )?;
                }
                CommitEntry::Overwrite(entry) => {
                    write!(
                        f,
                        "\nFailed to move temporary file to {}: {}",
                        entry.dest_path().display(),
                        failure.err
                    )?;
                }
            }
        }
        Ok(())
    }
}

fn apply_entry<FS: FileSystemOperations>(
    fs: &FS,
    dir_root: &AbsPath,
    temp_dir: &AbsPath,
    entry: CommitEntry,
    failed_entries: &mut Vec<FileRecoveryFailure>,
) {
    match &entry {
        CommitEntry::Overwrite(overwrite_entry) => {
            let temp_path = temp_dir.join(overwrite_entry.dest_path());
            let dest_path = dir_root.join(overwrite_entry.dest_path());
            match fs.rename_file_atomic(&temp_path, &dest_path) {
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
            let path = dir_root.join(delete_entry.path());
            match fs.remove_file(&path) {
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
}

fn check_root_path(fs: &impl FileSystemOperations, dir_root: &AbsPath) -> io::Result<()> {
    match fs.get_path_kind(dir_root)? {
        Some(PathKind::Directory) => {
            // Normal case, nothing to do.
            Ok(())
        }
        Some(PathKind::File) => io_bail!(
            NotADirectory,
            "Package root is a file: {}",
            dir_root.display()
        ),
        Some(PathKind::Other) => io_bail!(
            NotADirectory,
            "Package root is not a directory: {}",
            dir_root.display()
        ),
        None => io_bail!(
            NotFound,
            "Package root does not exist: {}",
            dir_root.display()
        ),
    }
}

pub(super) fn recover_path<FS: FileSystemOperations>(
    fs: &FS,
    dir_root: &AbsPath,
) -> io::Result<()> {
    check_root_path(fs, dir_root)?;

    let commit_path = dir_root.join_rel(RelPath::new_checked(COMMIT_PATH).unwrap());

    let Some(commit_file_kind) = fs.get_path_kind(&commit_path)? else {
        // No commit file, so no recovery needed.
        return Ok(());
    };

    match commit_file_kind {
        PathKind::File => {
            // We have some recovery to do. Continue.
        }
        PathKind::Directory => {
            io_bail!(InvalidData, "Commit file is a directory");
        }
        PathKind::Other => {
            io_bail!(InvalidData, "Commit file is not a regular file");
        }
    }

    let commit_data_bytes = fs.read_file(&commit_path, |data| {
        let limit = 128 * 1024 * 1024; // 128 MiB
        let mut data = LengthLimitedReader::new(data, limit);
        let mut buf = Vec::new();
        data.read_to_end(&mut buf)?;
        Ok(buf)
    })?;

    let mut commit_schema: CommitSchema = serde_json::from_slice(&commit_data_bytes)
        .map_err(io_err_map!(Other, "Failed to parse commit schema"))?;

    // We can support multiple versions in the future potentially, but for now
    // assume only the current version is valid.
    if commit_schema.version() != CURR_COMMIT_VERSION {
        io_bail!(
            InvalidData,
            "Unsupported commit schema version: {}",
            commit_schema.version()
        );
    }

    let abs_temp_dir = dir_root.join_rel(commit_schema.temp_dir());

    // Validate the paths in the commit schema.
    for entry in commit_schema.entries() {
        match entry {
            CommitEntry::Overwrite(entry) => {
                { is_valid_path(entry.dest_path(), commit_schema.temp_dir()) }?;
            }
            CommitEntry::Delete(entry) => {
                { is_valid_path(entry.path(), commit_schema.temp_dir()) }?;
            }
        }
    }

    {
        let mut failed_entries = Vec::new();

        for entry in commit_schema.take_entries() {
            apply_entry(fs, dir_root, &abs_temp_dir, entry, &mut failed_entries);
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
    let mut root_entries = fs.list_dir(dir_root)?;
    while let Some(entry) = root_entries
        .next()
        .transpose()
        .map_err(io_err_map!(Other, "Failed to read directory entry"))?
    {
        let entry_path = entry.path();
        if entry_path
            .file_name()
            .and_then(OsStr::to_str)
            .is_some_and(|name| name.starts_with("tmpdir-"))
        {
            // This is a temporary directory, remove it.
            fs.remove_dir_all(&entry_path).ok();
        }
    }

    fs.remove_file(&commit_path)?;

    Ok(())
}
