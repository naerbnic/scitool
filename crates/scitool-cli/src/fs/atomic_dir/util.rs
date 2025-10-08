use std::{
    io::{self, Write as _},
    path::{Component, Path, PathBuf},
};

const MANIFEST_FILE_NAME: &str = ".dir_manifest.json";

use cap_std::fs::{Dir, File};
use rand::distr::SampleString as _;

use crate::fs::{
    atomic_dir::DirLock,
    err_helpers::{io_bail, io_err_map},
    ops::WriteMode,
    paths::{RelPath, RelPathBuf, SinglePath, SinglePathBuf},
};

#[expect(dead_code, reason = "Primitive for current work")]
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
    if path.starts_with(MANIFEST_FILE_NAME) {
        io_bail!(
            Other,
            "Path must not start with the manifest file name: {}",
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

pub fn normalize_path(path: &Path) -> io::Result<RelPathBuf> {
    let path = RelPath::new_checked(path).map_err(io_err_map!(
        InvalidData,
        "Path is not a valid relative path: {}",
        path.display()
    ))?;
    let mut components = vec![];
    for component in path.components() {
        match component {
            Component::CurDir => {
                // Skip
            }
            Component::ParentDir => {
                if components.pop().is_none() {
                    io_bail!(
                        Other,
                        "Path must not contain a directory upreference above the root: {}",
                        path.display()
                    );
                }
            }
            Component::Normal(c) => {
                components.push(c.to_owned());
            }
            _ => {
                unreachable!("RelPath should not have Prefix or RootDir components");
            }
        }
    }
    Ok(RelPathBuf::try_from(PathBuf::from_iter(components)).expect("Components are all normal"))
}

pub fn safe_path_parent(path: &Path) -> io::Result<Option<(&Path, &Path)>> {
    let mut components = path.components();
    match components.next_back() {
        None => Ok(None),
        Some(Component::Normal(elem)) => Ok(Some((components.as_path(), elem.as_ref()))),
        Some(_) => io_bail!(
            Other,
            "Path must not end with a non-normal component: {}",
            path.display()
        ),
    }
}

pub(super) fn write_file_atomic(path: &Path, data: &[u8], write_mode: WriteMode) -> io::Result<()> {
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

struct TempFile<'a> {
    root: &'a Dir,
    file_name: SinglePathBuf,
    file: Option<cap_std::fs::File>,
}

impl<'a> TempFile<'a> {
    pub fn new_in(root: &'a Dir) -> io::Result<Self> {
        let file_name = format!(
            ".{}.tmp",
            rand::distr::Alphanumeric.sample_string(&mut rand::rng(), 10)
        );
        let file_name = SinglePathBuf::new_checked(&file_name)
            .map_err(io_err_map!(InvalidInput, "Generated file name is invalid"))?;
        let file = root.open_with(
            &file_name,
            cap_std::fs::OpenOptions::new().write(true).create_new(true),
        )?;
        Ok(TempFile {
            root,
            file_name,
            file: Some(file),
        })
    }

    pub(crate) fn persist(mut self, path: &Path) -> io::Result<File> {
        let path = SinglePath::new_checked(path).map_err(io_err_map!(Other, "Invalid path"))?;
        let file = self.file.take().expect("TempFile is valid");
        match self.root.rename(&self.file_name, self.root, path) {
            Ok(()) => Ok(file),
            Err(err) => {
                self.file = Some(file);
                Err(err)
            }
        }
    }

    pub(crate) fn persist_noclobber(mut self, path: &Path) -> io::Result<File> {
        let path = SinglePath::new_checked(path).map_err(io_err_map!(Other, "Invalid path"))?;
        let file = self.file.take().expect("TempFile is valid");
        match self.root.hard_link(&self.file_name, self.root, path) {
            Ok(()) => {
                self.root.remove_file(&self.file_name)?;
                Ok(file)
            }
            Err(err) => {
                self.file = Some(file);
                Err(err)
            }
        }
    }
}

impl std::ops::Deref for TempFile<'_> {
    type Target = cap_std::fs::File;

    fn deref(&self) -> &Self::Target {
        self.file.as_ref().expect("TempFile is valid")
    }
}

impl std::ops::DerefMut for TempFile<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.file.as_mut().expect("TempFile is valid")
    }
}

impl Drop for TempFile<'_> {
    fn drop(&mut self) {
        if let Some(_file) = self.file.take() {
            drop(self.root.remove_file(&self.file_name));
        }
    }
}

pub(super) fn write_file_atomic_at(
    root: &Dir,
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

    root.create_dir_all(parent)?;

    let mut temp_file = TempFile::new_in(root)?;
    {
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

pub(crate) fn create_old_path(dir_lock: &DirLock) -> SinglePathBuf {
    let suffix = rand::distr::Alphanumeric.sample_string(&mut rand::rng(), 6);
    let new_old_dir_name = format!("{}.old-{}", dir_lock.file_name().display(), suffix);
    SinglePathBuf::new_checked(&new_old_dir_name).expect("Generated file name should be valid")
}
