use std::{
    fs::TryLockError,
    io,
    path::{Path, PathBuf},
};

use crate::fs::{
    err_helpers::{io_bail, io_err},
    file_lock::{
        LockType,
        ephemeral::{self, EphemeralFileLock},
    },
};

const LOCK_FILE_SUFFIX: &str = ".lock";

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

    #[expect(dead_code, reason = "Primitive for current work")]
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

    #[expect(dead_code, reason = "Primitive for current work")]
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
