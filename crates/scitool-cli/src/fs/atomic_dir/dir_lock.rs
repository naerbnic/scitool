use std::{
    fs::TryLockError,
    io,
    path::{Path, PathBuf},
};

use cap_std::fs::Dir;

use crate::fs::{
    err_helpers::io_err,
    file_lock::{
        LockType,
        ephemeral::{self, EphemeralFileLock},
    },
};

const LOCK_FILE_SUFFIX: &str = ".lock";

pub(super) struct DirLock {
    parent_dir: Dir,
    target_path: PathBuf,
    lock_type: LockType,
    lock_file: EphemeralFileLock,
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
        let path = std::fs::canonicalize(path)?;
        let parent_path = path
            .parent()
            .ok_or_else(|| io_err!(Other, "Path must have a parent: {}", path.display()))?;
        let file_name = path
            .file_name()
            .ok_or_else(|| io_err!(Other, "Path must have a file name: {}", path.display()))?;
        let parent_dir = Dir::open_ambient_dir(parent_path, cap_std::ambient_authority())?;

        let lock_file_path =
            path.with_file_name(format!("{}{}", file_name.display(), LOCK_FILE_SUFFIX));
        let lock_file = ephemeral::lock_file(
            &ephemeral::DirRelativePath::new(&parent_dir, &lock_file_path),
            lock_type,
        )?;
        Ok(Self {
            parent_dir,
            target_path: path.to_path_buf(),
            lock_type,
            lock_file,
        })
    }

    #[expect(dead_code, reason = "Primitive for current work")]
    pub(super) fn try_acquire(path: &Path, lock_type: LockType) -> Result<Self, TryLockError> {
        let path = std::fs::canonicalize(path).map_err(TryLockError::Error)?;
        let parent_path = path
            .parent()
            .ok_or_else(|| io_err!(Other, "Path must have a parent: {}", path.display()))
            .map_err(TryLockError::Error)?;
        let file_name = path
            .file_name()
            .ok_or_else(|| io_err!(Other, "Path must have a file name: {}", path.display()))
            .map_err(TryLockError::Error)?;
        let parent_dir = Dir::open_ambient_dir(parent_path, cap_std::ambient_authority())
            .map_err(TryLockError::Error)?;

        let lock_file_path =
            path.with_file_name(format!("{}{}", file_name.display(), LOCK_FILE_SUFFIX));
        let lock_file = ephemeral::try_lock_file(
            &ephemeral::DirRelativePath::new(&parent_dir, &lock_file_path),
            lock_type,
        )?;
        Ok(Self {
            parent_dir,
            target_path: path.to_path_buf(),
            lock_type,
            lock_file,
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

    pub(super) fn upgrade(self) -> io::Result<Self> {
        let DirLock {
            parent_dir,
            target_path,
            lock_type,
            lock_file,
        } = self;
        Ok(Self {
            parent_dir,
            target_path,
            lock_type,
            lock_file: lock_file.upgrade()?,
        })
    }

    pub(super) fn downgrade(self) -> io::Result<Self> {
        let DirLock {
            parent_dir,
            target_path,
            lock_type,
            lock_file,
        } = self;
        Ok(Self {
            parent_dir,
            target_path,
            lock_type,
            lock_file: lock_file.downgrade()?,
        })
    }

    pub(super) fn parent_dir(&self) -> &Dir {
        &self.parent_dir
    }
}
