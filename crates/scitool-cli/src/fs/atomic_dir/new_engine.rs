#![expect(dead_code)]

use std::{
    ffi::OsString,
    io::Result,
    path::{Component, Path},
};

use cap_std::fs::Dir;
use cross_file_id::is_same_file;

use crate::fs::{
    err_helpers::{io_bail, io_err},
    file_lock::{LockFile, LockType},
};

const COMMIT_FILE_SUFFIX: &str = ".commit";
const LOCK_FILE_NAME: &str = ".dirlock";

fn are_dirs_equal(dir1: &Dir, dir2: &Dir) -> Result<bool> {
    is_same_file(dir1, dir2)
}

#[derive(Debug, Clone, Copy)]
enum OpenDirMode {
    Create,
    Open(LockType),
}

#[derive(Debug)]
struct DirHandle {
    /// The parent directory of the atomic directory.
    ///
    /// This is kept open to allow atomic renames within the same filesystem.
    parent_dir: Dir,

    /// The name of the root dir in the parent dir.
    ///
    /// This needs to be verified at time of use, to ensure that the directory
    /// has not been replaced by a symlink or moved.
    root_dir_name: OsString,

    /// The root directory of the atomic directory.
    root_dir: Dir,

    /// The lock file for the atomic directory. This can be either exclusive or shared,
    /// depending on whether the directory is opened for writing or reading.
    lock: LockFile,
}

impl DirHandle {
    fn open_impl(dir_path: &Path, open_mode: OpenDirMode, should_block: bool) -> Result<Self> {
        let (create, lock_mode) = match open_mode {
            OpenDirMode::Create => (true, LockType::Exclusive),
            OpenDirMode::Open(lock_mode) => (false, lock_mode),
        };

        let (parent_dir, root_dir, root_dir_name) = if create {
            // We don't expect the directory to exist when creating it, so when
            // we canonicalize, we have to remove the last segment.
            let dir_name_component = dir_path.components().next_back().ok_or_else(|| {
                io_err!(Other, "Must be able to find a dir name for new directory")
            })?;

            let Component::Normal(dir_name) = dir_name_component else {
                io_bail!(
                    Other,
                    "Cannot create atomic directory that ends in special component: {dir_name_component:?}"
                );
            };

            let Some(parent) = dir_path.parent() else {
                io_bail!(Other, "Cannot create atomic directory with an empty path");
            };

            let parent_dir = Dir::open_ambient_dir(parent, cap_std::ambient_authority())?;
            parent_dir.create_dir(dir_name)?;
            let root_dir = parent_dir.open_dir(dir_name)?;
            (parent_dir, root_dir, dir_name.to_os_string())
        } else {
            let root_dir = Dir::open_ambient_dir(dir_path, cap_std::ambient_authority())?;
            let dir_path = dir_path.canonicalize()?;
            let parent_dir =
                Dir::open_ambient_dir(dir_path.join(".."), cap_std::ambient_authority())?;
            let dir_name = dir_path
                .file_name()
                .ok_or_else(|| io_err!(Other, "Path must have a file name"))?;

            // Check that the root_dir is the same file as the one with the child in the parent.
            //
            // This is a bit of a pain, since same_file consumes a File. We use try_clone to
            // avoid consuming the original file handle.
            let subdir_file = parent_dir.open_dir(dir_name)?.into_std_file();

            let cloned_root_file = root_dir.try_clone()?.into_std_file();

            let subdir_file_handle = cross_file_id::Handle::from_file(subdir_file)?;
            let cloned_root_file_handle = cross_file_id::Handle::from_file(cloned_root_file)?;
            if subdir_file_handle != cloned_root_file_handle {
                io_bail!(
                    Other,
                    "Directory path does not point to the expected directory"
                );
            }

            (parent_dir, root_dir, dir_name.to_os_string())
        };

        let lock = if create {
            LockFile::create_in(&root_dir, LOCK_FILE_NAME)?
        } else {
            LockFile::open_in(&root_dir, LOCK_FILE_NAME, lock_mode, should_block)?
        };

        Ok(Self {
            parent_dir,
            root_dir_name,
            root_dir,
            lock,
        })
    }

    pub(crate) fn open(dir_path: &Path, lock_state: LockType, should_block: bool) -> Result<Self> {
        Self::open_impl(dir_path, OpenDirMode::Open(lock_state), should_block)
    }

    pub(crate) fn create(dir_path: &Path, should_block: bool) -> Result<Self> {
        Self::open_impl(dir_path, OpenDirMode::Create, should_block)
    }

    pub(crate) fn downgrade(&mut self, should_block: bool) -> Result<()> {
        self.lock.downgrade(should_block)
    }

    pub(crate) fn upgrade(&mut self, should_block: bool) -> Result<()> {
        self.lock.upgrade(should_block)?;
        Ok(())
    }

    pub(crate) fn root(&self) -> &Dir {
        &self.root_dir
    }

    pub(crate) fn mark_as_written(&mut self) -> Result<()> {
        self.lock.mark_as_written()
    }

    pub(crate) fn atomic_replace(&mut self, from_dir: DirHandle) -> Result<()> {
        // Ensure the two directories are siblings.
        if !are_dirs_equal(&self.parent_dir, &from_dir.parent_dir)? {
            io_bail!(
                Other,
                "Cannot rename from a directory in a different parent directory"
            );
        }
        // Ensure the root dir is still the same as when we opened it.
        let current_root = self.parent_dir.open_dir(&self.root_dir_name)?;
        let current_root_file = current_root.into_std_file();
        let opened_root_file = self.root_dir.try_clone()?.into_std_file();

        let current_root_handle = cross_file_id::Handle::from_file(current_root_file)?;
        let opened_root_handle = cross_file_id::Handle::from_file(opened_root_file)?;

        if current_root_handle != opened_root_handle {
            io_bail!(
                Other,
                "Directory has changed since it was opened; cannot rename"
            );
        }

        drop(from_dir);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::ErrorKind;
    use tempfile::TempDir;

    const TEST_DIR_NAME: &str = "test_atomic_dir";

    /// Get a temp directory for testing (the dir itself already exists)
    fn create_existing_test_dir() -> (TempDir, std::path::PathBuf) {
        let temp = TempDir::new().expect("Failed to create temp dir");
        let test_path = temp.path().to_path_buf();
        (temp, test_path)
    }

    /// Get a path for a new directory to create (parent exists, final dir doesn't)
    fn create_new_test_dir_path() -> (TempDir, std::path::PathBuf) {
        let temp = TempDir::new().expect("Failed to create temp dir");
        let test_path = temp.path().join("new_dir");
        (temp, test_path)
    }

    #[test]
    fn test_dir_handle_create() -> Result<()> {
        let (_temp, test_path) = create_new_test_dir_path();

        // Create a new directory with exclusive lock
        let handle = DirHandle::create(&test_path, true)?;

        // Verify directory exists
        assert!(test_path.exists());
        assert!(test_path.is_dir());

        // Verify lock file exists
        let lock_path = test_path.join(LOCK_FILE_NAME);
        assert!(lock_path.exists());

        // Verify lock is exclusive
        assert!(handle.lock.is_exclusive());

        Ok(())
    }

    #[test]
    fn test_dir_handle_create_already_exists() -> Result<()> {
        let (_temp, test_path) = create_new_test_dir_path();

        // Create once
        let _handle1 = DirHandle::create(&test_path, true)?;

        // Try to create again - should fail
        let result = DirHandle::create(&test_path, true);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), ErrorKind::AlreadyExists);

        Ok(())
    }

    #[test]
    fn test_dir_handle_open_shared() -> Result<()> {
        let (_temp, test_path) = create_new_test_dir_path();

        // Create directory first
        let creator = DirHandle::create(&test_path, true)?;
        drop(creator);

        // Open with shared lock
        let handle = DirHandle::open(&test_path, LockType::Shared, true)?;

        // Verify lock is shared
        assert!(handle.lock.is_shared());

        Ok(())
    }

    #[test]
    fn test_dir_handle_open_exclusive() -> Result<()> {
        let (_temp, test_path) = create_new_test_dir_path();

        // Create directory first
        let creator = DirHandle::create(&test_path, true)?;
        drop(creator);

        // Open with exclusive lock
        let handle = DirHandle::open(&test_path, LockType::Exclusive, true)?;

        // Verify lock is exclusive
        assert!(handle.lock.is_exclusive());

        Ok(())
    }

    #[test]
    fn test_dir_handle_multiple_shared_locks() -> Result<()> {
        let (_temp, test_path) = create_new_test_dir_path();

        // Create directory
        let creator = DirHandle::create(&test_path, true)?;
        drop(creator);

        // Open multiple shared locks
        let handle1 = DirHandle::open(&test_path, LockType::Shared, true)?;
        let handle2 = DirHandle::open(&test_path, LockType::Shared, true)?;
        let handle3 = DirHandle::open(&test_path, LockType::Shared, true)?;

        // All should succeed
        assert!(handle1.lock.is_shared());
        assert!(handle2.lock.is_shared());
        assert!(handle3.lock.is_shared());

        Ok(())
    }

    #[test]
    fn test_dir_handle_exclusive_blocks_shared() -> Result<()> {
        let (_temp, test_path) = create_new_test_dir_path();

        // Create directory
        let creator = DirHandle::create(&test_path, true)?;
        drop(creator);

        // Hold exclusive lock
        let _exclusive = DirHandle::open(&test_path, LockType::Exclusive, true)?;

        // Try to acquire shared lock without blocking - should fail
        let result = DirHandle::open(&test_path, LockType::Shared, false);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), ErrorKind::WouldBlock);

        Ok(())
    }

    #[test]
    fn test_dir_handle_shared_blocks_exclusive() -> Result<()> {
        let (_temp, test_path) = create_new_test_dir_path();

        // Create directory
        let creator = DirHandle::create(&test_path, true)?;
        drop(creator);

        // Hold shared lock
        let _shared = DirHandle::open(&test_path, LockType::Shared, true)?;

        // Try to acquire exclusive lock without blocking - should fail
        let result = DirHandle::open(&test_path, LockType::Exclusive, false);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), ErrorKind::WouldBlock);

        Ok(())
    }

    #[test]
    fn test_dir_handle_mark_as_written_shared_fails() -> Result<()> {
        let (_temp, test_path) = create_new_test_dir_path();

        // Create directory
        let creator = DirHandle::create(&test_path, true)?;
        drop(creator);

        // Open with shared lock
        let mut handle = DirHandle::open(&test_path, LockType::Shared, true)?;

        // Try to mark as written - should fail
        let result = handle.mark_as_written();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), ErrorKind::Other);

        Ok(())
    }

    #[test]
    fn test_dir_handle_root_access() -> Result<()> {
        let (_temp, test_path) = create_new_test_dir_path();

        let handle = DirHandle::create(&test_path, true)?;

        // Create a file in the root
        handle.root().write("test.txt", b"hello world")?;

        // Read it back
        let contents = handle.root().read("test.txt")?;
        assert_eq!(contents, b"hello world");

        Ok(())
    }
}
