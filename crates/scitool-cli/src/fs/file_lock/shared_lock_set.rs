use std::{
    collections::{HashMap, hash_map::Entry},
    fs::{File, TryLockError},
    io,
    sync::{Arc, Condvar, LazyLock, Mutex},
};

use same_file::Handle as SameFileHandle;

type LockMap = HashMap<Arc<SameFileHandle>, LockEntryState>;

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
    handle: Arc<SameFileHandle>,
    lock_type: LockType,
}

impl Lock {
    pub(super) fn lock_type(&self) -> LockType {
        self.lock_type
    }

    pub(super) fn into_file(self) -> File {
        // It really should be the case that same_file::Handle should be
        // convertible back to File, but it doesn't seem to be possible.
        // So we just clone the file handle.
        self.handle
            .as_file()
            .try_clone()
            .expect("Failed to clone file")
        // self will be dropped here, releasing the lock
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let mut lock_map = SHARED_HANDLE_LOCK_STATES.lock_map.lock().unwrap();
        let Some(entry) = lock_map.get_mut(&self.handle) else {
            panic!("Dropping a lock that is not held");
        };
        match entry {
            LockEntryState::Shared { ref_count } => {
                *ref_count -= 1;
                if *ref_count == 0 {
                    self.handle.as_file().unlock().expect("Unlock failed");
                    lock_map.remove(&self.handle);
                    SHARED_HANDLE_LOCK_STATES.waiters.notify_all();
                }
            }
            LockEntryState::Exclusive => {
                self.handle.as_file().unlock().expect("Unlock failed");
                lock_map.remove(&self.handle);
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
    fn lock_handle(&self, file: &File, lock_type: LockType) -> io::Result<Lock> {
        let handle = Arc::new(SameFileHandle::from_file(file.try_clone()?)?);
        let pending = {
            let mut lock_map = self.lock_map.lock().unwrap();
            loop {
                match lock_map.entry(handle.clone()) {
                    Entry::Occupied(mut occ) => {
                        if let (LockEntryState::Shared { ref_count }, LockType::Shared) =
                            (occ.get_mut(), lock_type)
                        {
                            // The process already has the lock, so just increment the ref count.
                            *ref_count += 1;

                            // Returning None indicates that no further action is needed.
                            return Ok(Lock {
                                handle,
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
            LockType::Exclusive => file.lock()?,
            LockType::Shared => file.lock_shared()?,
        }

        Ok(pending.accept_lock())
    }

    fn try_lock_handle(&self, file: &File, lock_type: LockType) -> Result<Lock, TryLockError> {
        let handle = Arc::new(
            SameFileHandle::from_file(file.try_clone().map_err(TryLockError::Error)?)
                .map_err(TryLockError::Error)?,
        );
        let pending = {
            let mut lock_map = self.lock_map.lock().unwrap();
            match lock_map.entry(handle.clone()) {
                Entry::Occupied(mut occ) => {
                    match occ.get_mut() {
                        LockEntryState::Shared { ref_count } if lock_type == LockType::Shared => {
                            // The process already has the lock, so just increment the ref count.
                            *ref_count += 1;

                            // Returning None indicates that no further action is needed.
                            return Ok(Lock {
                                handle,
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
            LockType::Exclusive => file.try_lock()?,
            LockType::Shared => file.try_lock_shared()?,
        }

        Ok(pending.accept_lock())
    }
}

struct PendingGuard {
    pending_handle: Option<Arc<SameFileHandle>>,
    lock_type: LockType,
}

impl PendingGuard {
    pub(self) fn accept_lock(mut self) -> Lock {
        let handle = self.pending_handle.take().expect("Lock already accepted");
        let lock_type = self.lock_type;
        let mut lock_map = SHARED_HANDLE_LOCK_STATES.lock_map.lock().unwrap();
        match lock_map.get_mut(&handle).expect("Pending lock missing") {
            LockEntryState::Pending {
                lock_type: pending_type,
            } => {
                assert!(
                    *pending_type == lock_type,
                    "Lock type changed while pending"
                );
                match lock_type {
                    LockType::Exclusive => {
                        *lock_map.get_mut(&handle).unwrap() = LockEntryState::Exclusive;
                    }
                    LockType::Shared => {
                        *lock_map.get_mut(&handle).unwrap() =
                            LockEntryState::Shared { ref_count: 1 };
                    }
                }
            }
            _ => panic!("Pending lock in unexpected state"),
        }
        Lock { handle, lock_type }
    }
}

impl Drop for PendingGuard {
    fn drop(&mut self) {
        if let Some(handle) = self.pending_handle.take() {
            let mut lock_map = SHARED_HANDLE_LOCK_STATES.lock_map.lock().unwrap();
            match lock_map.get_mut(&handle) {
                Some(LockEntryState::Pending { .. }) => {
                    lock_map.remove(&handle);
                    SHARED_HANDLE_LOCK_STATES.waiters.notify_all();
                }
                _ => panic!("Pending lock in unexpected state on drop"),
            }
        }
    }
}

pub(super) fn lock_file(file: &File, lock_type: LockType) -> io::Result<Lock> {
    SharedLockSet::instance().lock_handle(file, lock_type)
}

pub(super) fn try_lock_file(file: &File, lock_type: LockType) -> Result<Lock, TryLockError> {
    SharedLockSet::instance().try_lock_handle(file, lock_type)
}
