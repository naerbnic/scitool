use std::{
    collections::{HashMap, hash_map::Entry},
    fs::{File, TryLockError},
    io,
    sync::{Condvar, LazyLock, Mutex},
};

use cross_file_id::{FileId, Handle as SameFileHandle};

type LockMap = HashMap<FileId, LockEntryState>;

static SHARED_HANDLE_LOCK_STATES: LazyLock<SharedLockSet> = LazyLock::new(SharedLockSet::new);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LockType {
    Shared,
    Exclusive,
}

impl LockType {
    #[must_use]
    pub fn is_exclusive(self) -> bool {
        matches!(self, LockType::Exclusive)
    }
}

struct SharedLockSet {
    lock_map: Mutex<LockMap>,
    waiters: Condvar,
}

enum LockEntryState {
    Pending { lock_type: LockType },
    Exclusive,
    Shared { ref_count: usize },
}

#[derive(Debug)]
pub(super) struct Lock {
    // Precondition: This is Some at all times other than during or just before
    // a drop, and only within private code.
    handle: Option<SameFileHandle<File>>,
    lock_type: LockType,
}

impl Lock {
    pub(super) fn lock_type(&self) -> LockType {
        self.lock_type
    }

    pub(super) fn into_file(mut self) -> File {
        // It really should be the case that same_file::Handle should be
        // convertible back to File, but it doesn't seem to be possible.
        // So we just clone the file handle.
        let handle = self.handle.take().expect("Lock already taken");
        let file_id = SameFileHandle::id(&handle);
        // Remove the lock
        SHARED_HANDLE_LOCK_STATES
            .unlock_handle(&handle, &file_id)
            .expect("Unlock failed");

        // self will be dropped here, releasing the lock
        SameFileHandle::into_inner(handle)
    }
}

impl std::ops::Deref for Lock {
    type Target = File;

    fn deref(&self) -> &Self::Target {
        self.handle.as_ref().expect("Lock already taken")
    }
}

impl std::ops::DerefMut for Lock {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.handle.as_mut().expect("Lock already taken")
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let Some(handle) = self.handle.take() else {
            // If the lock was never fully acquired, do nothing.
            return;
        };
        let mut lock_map = SHARED_HANDLE_LOCK_STATES.lock_map.lock().unwrap();
        let file_id = SameFileHandle::id(&handle);
        let Some(entry) = lock_map.get_mut(&file_id) else {
            panic!("Dropping a lock that is not held");
        };
        match entry {
            LockEntryState::Shared { ref_count } => {
                *ref_count -= 1;
                if *ref_count == 0 {
                    handle.unlock().expect("Unlock failed");
                    lock_map.remove(&file_id);
                    SHARED_HANDLE_LOCK_STATES.waiters.notify_all();
                }
            }
            LockEntryState::Exclusive => {
                handle.unlock().expect("Unlock failed");
                lock_map.remove(&file_id);
                SHARED_HANDLE_LOCK_STATES.waiters.notify_all();
            }
            LockEntryState::Pending { .. } => {
                panic!("Dropping a pending lock");
            }
        }
    }
}

impl SharedLockSet {
    fn instance() -> &'static Self {
        &SHARED_HANDLE_LOCK_STATES
    }

    fn new() -> Self {
        Self {
            lock_map: Mutex::new(HashMap::new()),
            waiters: Condvar::new(),
        }
    }

    /// Attempt to take a lock on the given file handle with the specified lock type.
    /// If `block` is true, this will block until the lock can be taken.
    ///
    /// If the file is not currently locked in any way, a guard will be returned that holds a
    /// "pending" lock on the file. If it is dropped, the lock will be released. You can call
    /// `take_handle_lock` on the guard to convert the pending lock into a real lock.
    fn lock_handle(&self, file: File, lock_type: LockType) -> io::Result<Lock> {
        let handle = SameFileHandle::from_file(file)?;
        let file_id = SameFileHandle::id(&handle);
        let pending = {
            let mut lock_map = self.lock_map.lock().unwrap();
            loop {
                match lock_map.entry(file_id.clone()) {
                    Entry::Occupied(mut occ) => {
                        if let (LockEntryState::Shared { ref_count }, LockType::Shared) =
                            (occ.get_mut(), lock_type)
                        {
                            // The process already has the lock, so just increment the ref count.
                            *ref_count += 1;

                            // Returning None indicates that no further action is needed.
                            return Ok(Lock {
                                handle: Some(handle),
                                lock_type: LockType::Shared,
                            });
                        }
                    }
                    Entry::Vacant(vac) => {
                        // Indicate our intention to take the lock.
                        vac.insert(LockEntryState::Pending { lock_type });
                        break PendingGuard {
                            pending_handle: Some(handle),
                            lock_type,
                        };
                    }
                }
                // Wait for the lock to become available.
                lock_map = self.waiters.wait(lock_map).unwrap();
            }
        };

        match lock_type {
            LockType::Exclusive => pending.handle().lock()?,
            LockType::Shared => pending.handle().lock_shared()?,
        }

        Ok(pending.accept_lock())
    }

    fn try_lock_handle(&self, file: File, lock_type: LockType) -> Result<Lock, TryLockError> {
        let handle = SameFileHandle::from_file(file).map_err(TryLockError::Error)?;
        let file_id = SameFileHandle::id(&handle);
        let pending = {
            let mut lock_map = self.lock_map.lock().unwrap();
            match lock_map.entry(file_id.clone()) {
                Entry::Occupied(mut occ) => {
                    match occ.get_mut() {
                        LockEntryState::Shared { ref_count } if lock_type == LockType::Shared => {
                            // The process already has the lock, so just increment the ref count.
                            *ref_count += 1;

                            // Returning None indicates that no further action is needed.
                            return Ok(Lock {
                                handle: Some(handle),
                                lock_type: LockType::Shared,
                            });
                        }
                        _ => return Err(TryLockError::WouldBlock),
                    }
                }
                Entry::Vacant(vac) => {
                    // Indicate our intention to take the lock.
                    vac.insert(LockEntryState::Pending { lock_type });
                    PendingGuard {
                        pending_handle: Some(handle),
                        lock_type,
                    }
                }
            }
        };

        match lock_type {
            LockType::Exclusive => pending.handle().try_lock()?,
            LockType::Shared => pending.handle().try_lock_shared()?,
        }

        Ok(pending.accept_lock())
    }

    fn unlock_handle(&self, file: &File, file_id: &FileId) -> io::Result<()> {
        let mut lock_map = self.lock_map.lock().unwrap();
        let Some(entry) = lock_map.get_mut(file_id) else {
            panic!("Dropping a lock that is not held");
        };
        match entry {
            LockEntryState::Shared { ref_count } => {
                *ref_count -= 1;
                if *ref_count == 0 {
                    file.unlock()?;
                    lock_map.remove(file_id);
                    SHARED_HANDLE_LOCK_STATES.waiters.notify_all();
                }
            }
            LockEntryState::Exclusive => {
                file.unlock().expect("Unlock failed");
                lock_map.remove(file_id);
                SHARED_HANDLE_LOCK_STATES.waiters.notify_all();
            }
            LockEntryState::Pending { .. } => {
                panic!("Dropping a pending lock");
            }
        }
        Ok(())
    }
}

struct PendingGuard {
    pending_handle: Option<SameFileHandle<File>>,
    lock_type: LockType,
}

impl PendingGuard {
    pub(self) fn accept_lock(mut self) -> Lock {
        let handle = self.pending_handle.take().expect("Lock already accepted");
        let file_id = SameFileHandle::id(&handle);
        let lock_type = self.lock_type;
        let mut lock_map = SHARED_HANDLE_LOCK_STATES.lock_map.lock().unwrap();
        let entry = lock_map.get_mut(&file_id).expect("Pending lock missing");

        let LockEntryState::Pending {
            lock_type: pending_type,
        } = entry
        else {
            panic!("Pending lock in unexpected state");
        };

        assert!(
            *pending_type == lock_type,
            "Lock type changed while pending"
        );
        match lock_type {
            LockType::Exclusive => {
                *entry = LockEntryState::Exclusive;
            }
            LockType::Shared => {
                *entry = LockEntryState::Shared { ref_count: 1 };
            }
        }
        Lock {
            handle: Some(handle),
            lock_type,
        }
    }

    pub(self) fn handle(&self) -> &SameFileHandle<File> {
        self.pending_handle.as_ref().expect("Lock already accepted")
    }
}

impl Drop for PendingGuard {
    fn drop(&mut self) {
        if let Some(handle) = self.pending_handle.take() {
            let mut lock_map = SHARED_HANDLE_LOCK_STATES.lock_map.lock().unwrap();
            let file_id = SameFileHandle::id(&handle);
            match lock_map.get_mut(&file_id) {
                Some(LockEntryState::Pending { .. }) => {
                    lock_map.remove(&file_id);
                    SHARED_HANDLE_LOCK_STATES.waiters.notify_all();
                }
                _ => panic!("Pending lock in unexpected state on drop"),
            }
        }
    }
}

pub(super) fn lock_file(file: File, lock_type: LockType) -> io::Result<Lock> {
    SharedLockSet::instance().lock_handle(file, lock_type)
}

pub(super) fn try_lock_file(file: File, lock_type: LockType) -> Result<Lock, TryLockError> {
    SharedLockSet::instance().try_lock_handle(file, lock_type)
}
