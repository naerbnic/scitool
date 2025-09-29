use std::{
    io,
    path::{Component, Path},
};

use crate::fs::{
    atomic_dir::{COMMIT_PATH, LOCK_PATH},
    err_helpers::{io_bail, io_err_map},
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

pub(super) fn write_file_atomic<F, FS>(
    fs: &FS,
    base_dir: &AbsPath,
    tmp_dir: &AbsPath,
    write_mode: &WriteMode,
    path: &RelPath,
    body: F,
) -> io::Result<()>
where
    F: FnOnce(FS::FileWriter) -> io::Result<()>,
    FS: FileSystemOperations,
{
    let temp_path = tmp_dir.join_rel(path);
    let dest_path = base_dir.join_rel(path);
    // Create the parent directories for the given path.
    if let Some(parent) = temp_path.parent() {
        fs.create_dir_all(parent)?;
    }

    fs.write_to_file(WriteMode::CreateNew, &tmp_dir.join(path), body)?;

    // Create the parent directories for the destination path, if needed.
    if let Some(parent) = dest_path.parent() {
        fs.create_dir_all(parent)?;
    }

    match write_mode {
        // This will replace the destination file if it exists, but the change in the file data will
        // be atomic.
        WriteMode::Overwrite => fs.rename_file_atomic(&temp_path, &dest_path)?,

        // This will do an atomic creation of the destination file, so only one attempt to
        // create the file will succeed.
        WriteMode::CreateNew => {
            fs.link_file_atomic(&temp_path, &dest_path)?;
            // If the link succeeded, we can remove the temporary file.
            fs.remove_file(&temp_path)?;
        }
    }

    Ok(())
}
