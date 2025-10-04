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

use same_file::Handle as SameFileHandle;

use crate::fs::{
    err_helpers::{io_bail, io_err},
    file_lock::shared_lock_set,
};

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

    // To follow the protocol, we need to be able to reliably open and unlink
    // the same file. This trait abstracts over the two ways we support doing that
    pub(super) trait FileManager {
        fn open_file(&self, mode: LockOpenMode) -> io::Result<File>;
        fn unlink_file(&self) -> io::Result<()>;
    }

    // Note: This needs to be public, as it's in the public API of lock_file()
    // and try_lock_file(), functions, but it should not be used by clients.
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
        T: AsRef<Path> + ?Sized,
    {
        fn to_file_manager(&self) -> io::Result<LockFileManager> {
            let path = self.as_ref().to_owned();
            Ok(LockFileManager::new(path))
        }
    }

    impl<P> AsFileManager for DirRelativePath<'_, P>
    where
        P: AsRef<Path> + ?Sized,
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

const MAX_RETRIES: usize = 10000;

#[derive(Debug, thiserror::Error)]
enum SafeLockError {
    #[error("Nonblocking was requested, and the file is already locked")]
    WouldBlock,
    #[error("The lock file was deleted while trying to acquire the lock")]
    Deleted(std::fs::File),
    #[error(transparent)]
    Error(#[from] io::Error),
}

impl From<TryLockError> for SafeLockError {
    fn from(value: TryLockError) -> Self {
        match value {
            TryLockError::WouldBlock => SafeLockError::WouldBlock,
            TryLockError::Error(e) => SafeLockError::Error(e),
        }
    }
}

fn take_lock_safe(
    lock_file: std::fs::File,
    manager: &sealed::LockFileManager,
    lock_type: LockType,
    block: bool,
) -> Result<shared_lock_set::Lock, SafeLockError> {
    // Take the expected lock on the file. This path is for blocking locks.
    let lock = if block {
        shared_lock_set::lock_file(&lock_file, lock_type)?
    } else {
        shared_lock_set::try_lock_file(&lock_file, lock_type)?
    };

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
            let lock_file = manager.open_file(LockOpenMode::OpenOrCreate)?;
            return Err(SafeLockError::Deleted(lock_file));
        }
        Err(e) => return Err(e.into()),
    };
    // This is not particularly efficient, but it should be quick
    // enough that it doesn't matter.

    let lock_file_handle = SameFileHandle::from_file(lock_file)?;
    let current_file_handle = SameFileHandle::from_file(current_file.try_clone()?)?;
    if lock_file_handle == current_file_handle {
        // We verified that the file we locked is the current file at the path.
        // If other clients are well-behaved, the won't change the file without
        // taking an exclusive lock, so it shouldn't be removed out from under us.
        return Ok(lock);
    }

    Err(SafeLockError::Deleted(current_file))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LockOpenMode {
    OpenOrCreate,
    OpenExisting,
}

pub use shared_lock_set::LockType;

pub struct DirRelativePath<'a, P>
where
    P: ?Sized,
{
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

fn open_lock_file_impl<P>(
    path: &P,
    lock_type: LockType,
    block: bool,
) -> Result<EphemeralFileLock, TryLockError>
where
    P: sealed::AsFileManager + ?Sized,
{
    let manager = path.to_file_manager().map_err(TryLockError::Error)?;

    // Initial opening of the file. This should succeed unless there is a
    // permissions issue or similar.
    let mut file = manager
        .open_file(LockOpenMode::OpenOrCreate)
        .map_err(TryLockError::Error)?;

    // In theory, this loop could spin forever if another process
    // keeps deleting and recreating the file. In practice, this makes progress
    // in the same sense that atomic operations make progress: If we don't make
    // progress, it's because another process is making progress.
    //
    // To be completely safe, we limit the number of retries.
    for _ in 0..MAX_RETRIES {
        match take_lock_safe(file, &manager, lock_type, block) {
            Ok(l) => {
                return Ok(EphemeralFileLock {
                    lock: Some(l),
                    manager,
                });
            }
            Err(SafeLockError::WouldBlock) => {
                // Nonblocking was requested, and the file is already locked.
                return Err(TryLockError::WouldBlock);
            }
            Err(SafeLockError::Error(e)) => return Err(TryLockError::Error(e)),
            Err(SafeLockError::Deleted(f)) => {
                // The file we locked was deleted out from under us.
                // We need to go through the process again.
                file = f;
                // The loop will continue here.
            }
        }
    }

    Err(TryLockError::Error(io_err!(
        TimedOut,
        "Failed to acquire lock after {} retries",
        MAX_RETRIES
    )))
}

pub fn lock_file<P>(path: &P, lock_type: LockType) -> io::Result<EphemeralFileLock>
where
    P: sealed::AsFileManager + ?Sized,
{
    Ok(open_lock_file_impl(path, lock_type, true)?)
}

pub fn try_lock_file<P>(path: &P, lock_type: LockType) -> Result<EphemeralFileLock, TryLockError>
where
    P: sealed::AsFileManager + ?Sized,
{
    open_lock_file_impl(path, lock_type, false)
}

pub struct EphemeralFileLock {
    lock: Option<shared_lock_set::Lock>,
    manager: sealed::LockFileManager,
}

impl EphemeralFileLock {
    pub fn upgrade(&mut self) -> io::Result<()> {
        if let Some(lock) = self.lock.take() {
            if let LockType::Exclusive = lock.lock_type() {
                self.lock = Some(lock);
                return Ok(());
            }
            // Release the shared lock and get the file handle back.
            let lock_file = lock.into_file();
            let new_lock = take_lock_safe(lock_file, &self.manager, LockType::Exclusive, true)
                .map_err(|e| match e {
                    SafeLockError::WouldBlock => io_err!(Other, "Unexpected WouldBlock"),
                    SafeLockError::Deleted(_) => io_err!(Other, "Unexpected Deleted"),
                    SafeLockError::Error(e) => e,
                })?;
            self.lock = Some(new_lock);
            Ok(())
        } else {
            io_bail!(InvalidData, "Inconsistent internal lock state");
        }
    }

    pub fn downgrade(&mut self) -> io::Result<()> {
        if let Some(lock) = self.lock.take() {
            if let LockType::Shared = lock.lock_type() {
                // Already a shared lock, nothing to do.
                self.lock = Some(lock);
                return Ok(());
            }
            // Release the exclusive lock and get the file handle back.
            let lock_file = lock.into_file();
            let new_lock = take_lock_safe(lock_file, &self.manager, LockType::Shared, true)
                .map_err(|e| match e {
                    SafeLockError::WouldBlock => io_err!(Other, "Unexpected WouldBlock"),
                    SafeLockError::Deleted(_) => io_err!(Other, "Unexpected Deleted"),
                    SafeLockError::Error(e) => e,
                })?;
            self.lock = Some(new_lock);
            Ok(())
        } else {
            io_bail!(InvalidData, "Inconsistent internal lock state");
        }
    }
}

impl Drop for EphemeralFileLock {
    fn drop(&mut self) {
        let Some(lock) = self.lock.take() else {
            // We don't have the lock anymore, so there's nothing to do.
            return;
        };

        // If we have an exclusive lock here, we could just delete the file
        // and still be following the protocol, but there is a chance that
        // this could create live locks. If another process is waiting for
        // a lock of any kind, when we delete the file, it will succeed but
        // fail the same file check. Then it will have to try again, which
        // if there is nontrivial contention basically makes it a free-for-all
        // who gets to the lock first.
        //
        // By always releasing our lock and then trying to take an exclusive
        // lock, we are effectively yielding to the OS to give it an opportunity
        // to give the lock to another waiting process. This should reduce
        // contention and make it more likely that waiters will get the lock
        // in a fair order.

        // Release the lock and get the file handle back.
        let lock_file = lock.into_file();

        let _new_lock = match take_lock_safe(lock_file, &self.manager, LockType::Exclusive, false) {
            Ok(lock) => lock,
            Err(SafeLockError::WouldBlock | SafeLockError::Deleted(_)) => {
                // One or more other processes are using the file or has deleted
                // it which allows another process to acquire the lock. We leave it to
                // one of them to clean it up later.
                return;
            }
            Err(SafeLockError::Error(e)) => {
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
