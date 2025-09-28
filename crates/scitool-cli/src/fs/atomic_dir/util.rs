use std::{
    io,
    path::{Component, Path},
};

use crate::fs::{
    atomic_dir::{COMMIT_PATH, LOCK_PATH},
    err_helpers::{io_bail, io_err_map},
    paths::RelPath,
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
