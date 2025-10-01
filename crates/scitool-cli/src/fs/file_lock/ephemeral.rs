//! Defines an ephemeral file protocol for file locking.
//!
//! This lock protocol has the following properties:
//! - Multiple processes can hold shared locks simultaneously.
//! - Only one process can hold an exclusive lock at a time.
//! - Multiple locks to the same file in the same process are treated as independent locks.
//! - In normal operation, lock files are deleted when the lock is released.
//! - Every process using the same lock file path (assuming the path is stable) will
//!   see the same lock.
//!
//! Operating System Assumptions:
//! - Unlinking a file does not affect existing file handles.
//! - There is a mechanism to see if two file handles refer to the exact same file.
//! - If any number of processes try to take an exclusive lock on a file, exactly one will succeed.
//! - Releasing a lock on a file will tend to allow another waiting process to acquire the lock.
//!
//! Operating System Non-Assumptions:
//! - Locks can be held on the process level. In-process locking is handled in userspace.

use std::{fs::TryLockError, io};

use crate::fs::file_lock::{LockType, shared_lock_set};

mod sealed {

    use crate::fs::err_helpers::io_bail;

    use super::{DirRelativePath, LockOpenMode};
    use std::{
        fs::{File, OpenOptions},
        io,
        path::{Path, PathBuf},
    };

    struct DirRelativePathOwned {
        dir: cap_std::fs::Dir,
        path: PathBuf,
    }

    pub(super) trait FileManager {
        fn open_file(&self, mode: LockOpenMode) -> io::Result<File>;
        fn unlink_file(&self) -> io::Result<()>;
    }

    // Note: This needs to be public, as it's in the public API of EphemeralLock,
    // but it should not be constructible outside this module.
    #[doc(hidden)]
    pub struct LockFileManager(Box<dyn FileManager + Send + Sync>);

    impl LockFileManager {
        fn new<T>(manager: T) -> Self
        where
            T: FileManager + Send + Sync + 'static,
        {
            Self(Box::new(manager))
        }
        pub(super) fn open_file(&self, mode: LockOpenMode) -> io::Result<File> {
            self.0.open_file(mode)
        }

        pub(super) fn unlink_file(&self) -> io::Result<()> {
            self.0.unlink_file()
        }
    }

    impl FileManager for PathBuf {
        fn open_file(&self, mode: LockOpenMode) -> io::Result<File> {
            let mut options = OpenOptions::new();
            match mode {
                LockOpenMode::OpenOrCreate => {
                    options.read(true).write(true).create(true).truncate(true)
                }
                LockOpenMode::OpenExisting => options.read(true).write(true),
            };
            options.open(self)
        }

        fn unlink_file(&self) -> io::Result<()> {
            match std::fs::remove_file(self) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(e),
            }
        }
    }

    impl FileManager for DirRelativePathOwned {
        fn open_file(&self, mode: LockOpenMode) -> io::Result<File> {
            let mut options = cap_std::fs::OpenOptions::new();
            match mode {
                LockOpenMode::OpenOrCreate => {
                    options.read(true).write(true).create(true).truncate(true)
                }
                LockOpenMode::OpenExisting => options.read(true).write(true),
            };

            self.dir
                .open_with(&self.path, &options)
                .map(cap_std::fs::File::into_std)
        }

        fn unlink_file(&self) -> io::Result<()> {
            match self.dir.remove_file(&self.path) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(e),
            }
        }
    }

    pub trait AsFileManager {
        fn to_file_manager(&self) -> io::Result<LockFileManager>;
    }

    impl<T> AsFileManager for T
    where
        T: AsRef<Path>,
    {
        fn to_file_manager(&self) -> io::Result<LockFileManager> {
            let path = self.as_ref().to_owned();
            Ok(LockFileManager::new(path))
        }
    }

    impl<P> AsFileManager for DirRelativePath<'_, P>
    where
        P: AsRef<Path>,
    {
        fn to_file_manager(&self) -> io::Result<LockFileManager> {
            let dir = self.dir.try_clone()?;
            let path = self.path.as_ref().to_owned();
            if path.is_absolute() {
                io_bail!(InvalidFilename, "Path must be relative");
            }
            Ok(LockFileManager::new(DirRelativePathOwned { dir, path }))
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LockOpenMode {
    OpenOrCreate,
    OpenExisting,
}

pub struct DirRelativePath<'a, P> {
    dir: &'a cap_std::fs::Dir,
    path: &'a P,
}

impl<'a, P> DirRelativePath<'a, P>
where
    P: AsRef<std::path::Path>,
{
    pub fn new(dir: &'a cap_std::fs::Dir, path: &'a P) -> Self {
        Self { dir, path }
    }
}

pub fn open_lock_file<P>(path: &P, lock_type: LockType) -> io::Result<EphemeralFileLock>
where
    P: sealed::AsFileManager,
{
    let manager = path.to_file_manager()?;

    // Initial opening of the file. This should succeed unless there is a
    // permissions issue or similar.
    let mut file = manager.open_file(LockOpenMode::OpenOrCreate)?;

    // In theory, this loop could spin forever if another process
    // keeps deleting and recreating the file. In practice, this makes progress
    // in the same sense that atomic operations make progress: If we don't make
    // progress, it's because another process is making progress.
    loop {
        // Take the expected lock on the file. This path is for blocking locks.
        let lock = shared_lock_set::lock_file(&file, lock_type)?;

        // We have the lock we expect, but it's possible that the file we
        // locked is not the current file at the path. This can happen
        // if another process deleted and recreated the file between when
        // we opened it and when we locked it. We need to verify that
        // the file we locked is still the current file at the path.
        let current_file = match manager.open_file(LockOpenMode::OpenExisting) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                // The file we locked was deleted out from under us.
                // We need to go through the process again.
                file = manager.open_file(LockOpenMode::OpenOrCreate)?;
                continue;
            }
            Err(e) => return Err(e),
        };
        // This is not particularly efficient, but it should be quick
        // enough that it doesn't matter.

        let lock_file_handle = same_file::Handle::from_file(file)?;
        let current_file_handle = same_file::Handle::from_file(current_file.try_clone()?)?;
        if lock_file_handle == current_file_handle {
            // We verified that the file we locked is the current file at the path.
            // If other clients are well-behaved, the won't change the file without
            // taking an exclusive lock, so it shouldn't be removed out from under us.
            return Ok(EphemeralFileLock {
                lock: Some(lock),
                manager,
            });
        }
        // The file we locked is not the current file at the path.
        // We need to go through the process again.
        //
        // This should be a rare occurrence, so we don't need to worry about
        // performance too much here.
        //
        // The current_file we opened is a new candidate for the file to lock.
        file = current_file;
    }
}

pub struct EphemeralFileLock {
    lock: Option<shared_lock_set::Lock>,
    manager: sealed::LockFileManager,
}

impl Drop for EphemeralFileLock {
    fn drop(&mut self) {
        let Some(lock) = self.lock.take() else {
            // We don't have the lock anymore, so there's nothing to do.
            return;
        };

        // The cleanup logic requires that we drop our lock and try once to
        // take an exclusive lock on the file. If we can take the exclusive
        // lock, we know that no other process is using the file, so we can
        // safely delete it. If we can't take the exclusive lock, we leave
        // the file alone, since another process is using it.

        let lock_file = lock.into_file();
        let _new_lock = match shared_lock_set::try_lock_file(&lock_file, LockType::Exclusive) {
            Ok(l) => l,
            Err(TryLockError::WouldBlock) => {
                // One or more other processes are using the file. We leave it to
                // one of them to clean it up later.
                return;
            }
            Err(TryLockError::Error(e)) => {
                // We had an unexpected error trying to take the lock.
                // We can't safely delete the file, so we leave it alone.
                panic!("File error occurred on lock acquisition: {e}")
            }
        };

        // Now we have the exclusive lock in _new_lock, so we can clean up after the file.
        //
        // Note that this requires the original path to refer to the same file
        // we locked, as the standard library doesn't have a crate to unlink a
        // file by handle.
        if let Err(e) = self.manager.unlink_file() {
            panic!("Failed to delete lock file: {e}");
        }
    }
}
