#![expect(unsafe_code)]
use parking_lot::{
    RawMutex, RawThreadId,
    lock_api::{GetThreadId as _, RawMutex as _},
};
use std::{
    cell::{RefCell, RefMut, UnsafeCell},
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicUsize, Ordering},
    thread,
};

/// A mutex that can provide a guard, but can also make a scoped lock, where
/// attempts to get the lock within the scope will always succeed.
///
/// # Motivation
///
/// We will have a comparison function for our indexes that will require us to
/// repeatedly lock and unlock the storage, which when used with mutexes are
/// likely to be very slow. By allowing us to create a single overall lock
/// that we can hold for the duration of the comparison, we can avoid this
/// issue.
///
/// We still want to follow the semantics of typical Mutexes, for a few
/// reasons:
///
/// 1. We want to be able to track whether the consumer is reading and/or
///    writing to the contents.
/// 2. We want to be able to track poisoning of the value. Even if the lock is
///    held in an outer scope, the lock won't be poisoned unless a panic occurs
///    while an explicit guard is held.
///
/// # Questions
///
/// - Do we want to have the same semantics for locks outside of a scope? If
///   a user calls `lock()` while a scope is _not_ held, should it take the
///   lock and return a guard, or should it panic?
/// - Do the above semantics cause any issues with usability?
pub(super) struct ScopeMutex<T> {
    mutex: RawThreadMutex,
    // Precondition: The contents of UnsafeCell are only accessed when the
    // current thread holds self.mutex.
    inner: UnsafeCell<Inner<T>>,
}

// As long as the contents T are send, and there are no borrows on the
// contents, we can send the mutex to another thread.
// unsafe impl<T: Send> Send for ScopeMutex<T> {}

unsafe impl<T: Send> Sync for ScopeMutex<T> {}

impl<T> ScopeMutex<T>
where
    T: Send,
{
    pub(super) fn new(contents: T) -> Self {
        Self {
            mutex: RawThreadMutex::new(),
            inner: UnsafeCell::new(Inner {
                poisoned: false,
                contents: RefCell::new(contents),
            }),
        }
    }

    /// Locks the mutex, making it local to the current thread.
    ///
    /// All attempts to lock the mutex while a scope is held will succeed without
    /// blocking, and with minimal atomic overhead.
    pub(super) fn lock_scope<F, R>(&self, scope: F) -> R
    where
        F: FnOnce() -> R,
    {
        let guard = self.mutex.lock();
        let result = scope();
        drop(guard);
        result
    }

    /// Locks the mutex, providing mutable access to the contents.
    ///
    /// If called within a scope, this re-uses the existing lock, but is
    /// not re-entrant. It still follows the same semantics of `std::sync::Mutex`.
    /// where calling `lock()` while there is another lock guard held by the
    /// current thread will panic.
    ///
    /// If called outside a scope, this acquires the lock (blocking).
    pub(super) fn lock(&self) -> ScopeMutexGuard<'_, T> {
        let owned_lock = if self.mutex.is_current_locked() {
            None
        } else {
            // Not in a scope (or at least not one we own). Acquire the lock.
            Some(self.mutex.lock())
        };

        // SAFETY: We hold the mutex lock, making us the sole accessor/owner of
        // the contents.
        let inner_mut: &mut Inner<T> = unsafe {
            self.inner
                .get()
                .as_mut()
                .expect("inner should never be null")
        };

        assert!(!inner_mut.poisoned, "ScopeMutex is poisoned");

        let borrow = inner_mut.contents.borrow_mut();

        ScopeMutexGuard {
            poisoned: &mut inner_mut.poisoned,
            borrow: Some(borrow),
            owned_lock,
        }
    }
}

struct RawThreadMutex {
    mutex: RawMutex,
    thread_id: AtomicUsize,
}

impl RawThreadMutex {
    fn new() -> Self {
        Self {
            mutex: RawMutex::INIT,
            thread_id: AtomicUsize::new(0),
        }
    }

    fn lock(&self) -> RawThreadMutexGuard<'_> {
        self.mutex.lock();
        self.thread_id.store(
            RawThreadId::INIT.nonzero_thread_id().get(),
            Ordering::SeqCst,
        );
        RawThreadMutexGuard { mutex: self }
    }

    fn is_current_locked(&self) -> bool {
        self.thread_id.load(Ordering::SeqCst) == RawThreadId::INIT.nonzero_thread_id().get()
    }
}

struct RawThreadMutexGuard<'a> {
    mutex: &'a RawThreadMutex,
}

impl Drop for RawThreadMutexGuard<'_> {
    fn drop(&mut self) {
        self.mutex.thread_id.store(0, Ordering::SeqCst);
        // SAFETY: We own the lock, so we must unlock it.
        unsafe {
            self.mutex.mutex.unlock();
        }
    }
}

struct Inner<T> {
    poisoned: bool,
    contents: RefCell<T>,
}

pub(super) struct ScopeMutexGuard<'a, T> {
    poisoned: &'a mut bool,
    borrow: Option<RefMut<'a, T>>,
    owned_lock: Option<RawThreadMutexGuard<'a>>,
}

impl<T> Deref for ScopeMutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.borrow.as_ref().unwrap()
    }
}

impl<T> DerefMut for ScopeMutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.borrow.as_mut().unwrap()
    }
}

impl<T> Drop for ScopeMutexGuard<'_, T> {
    fn drop(&mut self) {
        if thread::panicking() {
            *self.poisoned = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_basic_usage() {
        let mutex = ScopeMutex::new(5);
        mutex.lock_scope(|| {
            let mut guard = mutex.lock();
            *guard += 1;
        });

        mutex.lock_scope(|| {
            let guard = mutex.lock();
            assert_eq!(*guard, 6);
        });
    }

    #[test]
    fn test_standalone_lock() {
        let mutex = ScopeMutex::new(0);
        {
            let mut guard = mutex.lock();
            *guard += 1;
        }
        assert_eq!(*mutex.lock(), 1);
    }

    #[test]
    fn test_poisoning() {
        let mutex = Arc::new(ScopeMutex::new(0));
        let m2 = mutex.clone();

        // Use a separate thread to poison the mutex
        let handle = thread::spawn(move || {
            // We expect this to panic, so we catch it to prevent the test runner from aborting (though join handles that too usually)
            // Actually, we want the panic to unwind through the guard to trigger drop.
            // catch_unwind will stop the unwind, but the guard is inside it.
            let _result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                m2.lock_scope(|| {
                    let _guard = m2.lock();
                    panic!("oops");
                });
            }));
        });

        // Wait for thread to finish
        handle.join().unwrap();

        // lock_scope should succeed
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            mutex.lock_scope(|| {
                // lock should panic
                let _g = mutex.lock();
            });
        }));

        assert!(result.is_err(), "lock() should panic on poisoned mutex");
    }

    #[test]
    #[should_panic(expected = "RefCell already borrowed")]
    fn test_recursive_lock_panic() {
        let mutex = ScopeMutex::new(0);
        mutex.lock_scope(|| {
            let _g1 = mutex.lock();
            // This should panic now, even though we are in a scope
            let _g2 = mutex.lock();
        });
    }
}
