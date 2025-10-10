mod wait_queue;

use std::{
    collections::{HashMap, hash_map::Entry},
    fs::{File, TryLockError},
    io,
    sync::{LazyLock, Mutex},
};

use cross_file_id::{FileId, Handle as SameFileHandle};

use crate::shared_lock_set::wait_queue::WaitQueue;

type LockMap = HashMap<FileId, LockEntry>;

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
    // A lock_map that keeps track of the current locks held on file handles.
    //
    // The keys of the lock_map are FileIds, which are only valid while at
    // least one file handle to it is open in the process. It should be
    // removed from the map before it is closed/dropped.
    //
    // Invariant: There is an entry in the lock_map for each file handle
    // that either is currently locked, or is in the process of being locked
    // (i.e. a thread is waiting to acquire the lock).
    lock_map: Mutex<LockMap>,
}

struct LockEntry {
    queue: WaitQueue<LockResult>,
    state: LockEntryState,
}

enum LockEntryState {
    PendingFileAcquire { lock_type: LockType },
    PendingFileRelease,
    Exclusive,
    Shared { ref_count: usize },
}

#[derive(Debug)]
pub(super) struct Lock {
    // Invariant: This is Some at all times other than during or just before
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
        let file_id = SameFileHandle::id(&handle);
        SHARED_HANDLE_LOCK_STATES
            .unlock_handle(&handle, &file_id)
            .expect("Unlock failed");
    }
}

#[derive(Debug)]
enum LockResult {
    /// The current thread is responsible for taking the file lock, and returning it,
    /// and will be the first (or one of the first) to hold the lock.
    PendingFileLockAcquire(PendingGuard),
    /// The lock desired was successfully acquired.
    Acquired,
}

#[derive(Debug, Clone, Copy)]
enum UnlockResult {
    /// The last lock was released, and we should unlock file and report once
    /// the unlock is complete.
    PendingFileUnlock,
    /// The lock was released, and no further action is needed.
    Released,
}

impl SharedLockSet {
    fn instance() -> &'static Self {
        &SHARED_HANDLE_LOCK_STATES
    }

    fn new() -> Self {
        Self {
            lock_map: Mutex::new(HashMap::new()),
        }
    }

    fn take_lock(&self, file_id: &FileId, lock_type: LockType) -> LockResult {
        let waiter = 'lock_attempt: {
            let mut lock_map = self.lock_map.lock().unwrap();
            match lock_map.entry(file_id.clone()) {
                Entry::Vacant(vac) => {
                    let lock_entry = LockEntry {
                        queue: WaitQueue::new(),
                        state: LockEntryState::PendingFileAcquire { lock_type },
                    };
                    vac.insert(lock_entry);
                    return LockResult::PendingFileLockAcquire(PendingGuard {
                        pending_handle: Some(file_id.clone()),
                    });
                }
                Entry::Occupied(mut occ) => {
                    let entry = occ.get_mut();
                    if !entry.queue.is_empty() {
                        break 'lock_attempt entry.queue.push(lock_type);
                    }
                    match (&mut occ.get_mut().state, lock_type) {
                        (LockEntryState::Shared { ref_count }, LockType::Shared) => {
                            // The process already has the lock, so just increment the ref count.
                            *ref_count += 1;
                            return LockResult::Acquired;
                        }
                        _ => {
                            // We need to wait for the lock to become available.
                            break 'lock_attempt occ.get_mut().queue.push(lock_type);
                        }
                    }
                }
            }
        };

        // Wait for the lock to become available.
        //
        // Returns a value indicating the reason we were woken up.
        waiter.wait()
    }

    fn try_take_lock(&self, file_id: &FileId, lock_type: LockType) -> Option<LockResult> {
        let mut lock_map = self.lock_map.lock().unwrap();
        match lock_map.entry(file_id.clone()) {
            Entry::Vacant(vac) => {
                vac.insert(LockEntry {
                    queue: WaitQueue::new(),
                    state: LockEntryState::PendingFileAcquire { lock_type },
                });
                Some(LockResult::PendingFileLockAcquire(PendingGuard {
                    pending_handle: Some(file_id.clone()),
                }))
            }
            Entry::Occupied(mut occ) => {
                let entry = occ.get_mut();
                if !entry.queue.is_empty() {
                    return None;
                }
                match (&mut occ.get_mut().state, lock_type) {
                    (LockEntryState::Shared { ref_count }, LockType::Shared) => {
                        // The process already has the lock, so just increment the ref count.
                        *ref_count += 1;
                        Some(LockResult::Acquired)
                    }
                    _ => None,
                }
            }
        }
    }

    fn unlock(&self, file_id: &FileId) -> UnlockResult {
        let mut lock_map = self.lock_map.lock().unwrap();
        let Some(entry) = lock_map.get_mut(file_id) else {
            panic!("Dropping a lock that is not held");
        };

        match &mut entry.state {
            LockEntryState::Shared { ref_count } => {
                *ref_count -= 1;
                if *ref_count != 0 {
                    UnlockResult::Released
                } else {
                    entry.state = LockEntryState::PendingFileRelease;
                    UnlockResult::PendingFileUnlock
                }
            }
            LockEntryState::Exclusive => {
                // We will be releasing our exclusive lock, and there is no one waiting.
                entry.state = LockEntryState::PendingFileRelease;
                UnlockResult::PendingFileUnlock
            }
            _ => panic!("Dropping a lock that is not held"),
        }
    }

    fn on_file_unlock(&self, file_id: &FileId) {
        let mut lock_map = self.lock_map.lock().unwrap();
        let Some(entry) = lock_map.get_mut(file_id) else {
            panic!("Unlocking a file that is not held");
        };

        match entry.queue.next_mut() {
            None => {
                // No one is waiting for the lock, so remove the entry.
                lock_map.remove(file_id);
            }
            Some(next) => {
                let lock_type = next.lock_type();
                let leader = next.take_waiter();
                entry.state = LockEntryState::PendingFileAcquire { lock_type };
                leader.wake(LockResult::PendingFileLockAcquire(PendingGuard {
                    pending_handle: Some(file_id.clone()),
                }));
            }
        }
    }

    fn on_pending_file_lock_acquired(&self, file_id: &FileId) {
        let mut lock_map = self.lock_map.lock().unwrap();
        let Some(entry) = lock_map.get_mut(file_id) else {
            panic!("Locking a file that is not held");
        };

        let LockEntryState::PendingFileAcquire { lock_type } = entry.state else {
            panic!("Reporting a file lock that is not pending");
        };

        match lock_type {
            LockType::Exclusive => {
                entry.state = LockEntryState::Exclusive;
            }
            LockType::Shared => {
                let mut waiters = Vec::new();
                while let Some(next) = entry.queue.next_mut()
                    && next.lock_type() == LockType::Shared
                {
                    waiters.push(next.take_waiter());
                }

                entry.state = LockEntryState::Shared {
                    ref_count: 1 + waiters.len(),
                };
                for waiter in waiters {
                    waiter.wake(LockResult::Acquired);
                }
            }
        }
    }

    fn on_pending_file_lock_dropped(&self, file_id: &FileId) {
        let mut lock_map = self.lock_map.lock().unwrap();
        let Some(entry) = lock_map.get_mut(file_id) else {
            panic!("Dropping a lock that is not held");
        };
        let LockEntryState::PendingFileAcquire { .. } = entry.state else {
            panic!("Dropping a pending lock in the wrong state");
        };

        // We were pending, so there should be a wait group for us.
        if let Some(next) = entry.queue.next_mut() {
            let lock_type = next.lock_type();
            let new_leader = next.take_waiter();
            entry.state = LockEntryState::PendingFileAcquire { lock_type };
            new_leader.wake(LockResult::PendingFileLockAcquire(PendingGuard {
                pending_handle: Some(file_id.clone()),
            }));
        } else {
            // Well, we're out of waiters, so just remove the entry.
            lock_map.remove(file_id);
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
        match self.take_lock(&file_id, lock_type) {
            LockResult::PendingFileLockAcquire(pending) => {
                match lock_type {
                    LockType::Exclusive => handle.lock()?,
                    LockType::Shared => handle.lock_shared()?,
                }
                pending.accept_lock();
            }
            LockResult::Acquired => {
                // Nothing to do, we already have the lock.
            }
        }
        Ok(Lock {
            handle: Some(handle),
            lock_type,
        })
    }

    fn try_lock_handle(&self, file: File, lock_type: LockType) -> Result<Lock, TryLockError> {
        let handle = SameFileHandle::from_file(file).map_err(TryLockError::Error)?;
        let file_id = SameFileHandle::id(&handle);
        match self.try_take_lock(&file_id, lock_type) {
            None => return Err(TryLockError::WouldBlock),
            Some(LockResult::PendingFileLockAcquire(pending)) => {
                match lock_type {
                    LockType::Exclusive => handle.try_lock()?,
                    LockType::Shared => handle.try_lock_shared()?,
                }
                pending.accept_lock();
            }
            Some(LockResult::Acquired) => {
                // Nothing to do, we already have the lock.
            }
        }
        Ok(Lock {
            handle: Some(handle),
            lock_type,
        })
    }

    fn unlock_handle(&self, file: &File, file_id: &FileId) -> io::Result<()> {
        match self.unlock(file_id) {
            UnlockResult::PendingFileUnlock => {
                file.unlock()?;
                self.on_file_unlock(file_id);
            }
            UnlockResult::Released => {
                // Nothing to do, we still have the lock.
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
struct PendingGuard {
    pending_handle: Option<FileId>,
}

impl PendingGuard {
    pub(self) fn accept_lock(mut self) {
        SHARED_HANDLE_LOCK_STATES.on_pending_file_lock_acquired(
            &self.pending_handle.take().expect("Lock already accepted"),
        );
    }
}

impl Drop for PendingGuard {
    fn drop(&mut self) {
        if let Some(file_id) = self.pending_handle.take() {
            SHARED_HANDLE_LOCK_STATES.on_pending_file_lock_dropped(&file_id);
        }
    }
}

pub(super) fn lock_file(file: File, lock_type: LockType) -> io::Result<Lock> {
    SharedLockSet::instance().lock_handle(file, lock_type)
}

pub(super) fn try_lock_file(file: File, lock_type: LockType) -> Result<Lock, TryLockError> {
    SharedLockSet::instance().try_lock_handle(file, lock_type)
}
