#![expect(unsafe_code, reason = "needed for guard implementation")]
use parking_lot::{RawMutex, lock_api::RawMutex as _};
use std::{cell::UnsafeCell, rc::Rc};

use super::core::{Guard, GuardedOperation};

/// A mutex that simply tracks a lock state on a target value.
///
/// Unlike [`std::sync::Mutex`], this does not track the thread that holds the
/// lock, and is not poisoned on a panic.
pub(crate) struct PureMutex<T> {
    raw_mutex: RawMutex,
    contents: UnsafeCell<T>,
}

unsafe impl<T> Send for PureMutex<T> {}
unsafe impl<T> Sync for PureMutex<T> {}

impl<T> PureMutex<T>
where
    T: Send,
{
    #[cfg_attr(not(test), expect(dead_code, reason = "experimental"))]
    pub(crate) fn new(value: T) -> Self {
        Self {
            raw_mutex: RawMutex::INIT,
            contents: UnsafeCell::new(value),
        }
    }

    #[cfg_attr(not(test), expect(dead_code, reason = "experimental"))]
    pub(crate) fn lock(&self) -> PureMutexGuard<'_, T> {
        PureMutexGuard(Guard::lock(PureMutexOp::new(), self).unwrap())
    }
}

pub(crate) struct PureMutexOp<T> {
    _phantom: std::marker::PhantomData<Rc<T>>,
}

impl<T> PureMutexOp<T> {
    pub(crate) fn new() -> Self {
        PureMutexOp {
            _phantom: std::marker::PhantomData,
        }
    }
}

pub(crate) struct PureMutexGuard<'a, T>(Guard<'a, PureMutexOp<T>>);

impl<T> std::ops::Deref for PureMutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.contents()
    }
}

impl<T> std::ops::DerefMut for PureMutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.contents_mut()
    }
}

unsafe impl<T> GuardedOperation for PureMutexOp<T> {
    type Target<'a>
        = &'a PureMutex<T>
    where
        Self: 'a;

    type SharedTarget<'a>
        = &'a PureMutex<T>
    where
        Self: 'a;

    type Output<'a>
        = &'a T
    where
        T: 'a;

    type OutputMut<'a>
        = &'a mut T
    where
        T: 'a;
    type Error = std::convert::Infallible;

    fn acquire<'a>(&self, target: &mut Self::Target<'a>) -> Result<(), Self::Error>
    where
        Self: 'a,
    {
        target.raw_mutex.lock();
        Ok(())
    }

    fn try_acquire<'a>(&self, target: &mut Self::Target<'a>) -> Result<bool, Self::Error>
    where
        Self: 'a,
    {
        Ok(target.raw_mutex.try_lock())
    }

    unsafe fn release<'a>(&self, target: &mut Self::Target<'a>)
    where
        Self: 'a,
    {
        // Safety: The safety requirements of this method enforce
        // that acquire() or try_acquire() has been called on the target
        // value with the same operation value, and release() has not
        // been called yet.
        unsafe { target.raw_mutex.unlock() }
    }

    unsafe fn target_to_shared<'short>(
        &self,
        target: &'short Self::Target<'_>,
    ) -> Self::SharedTarget<'short> {
        // Safety: Caller guarantees lock is held.
        // We can access the UnsafeCell contents
        target
    }

    fn view<'a>(&self, target: Self::SharedTarget<'a>) -> Self::Output<'a> {
        // Safety: Caller guarantees lock is held.
        unsafe { &*target.contents.get() }
    }

    unsafe fn reborrow<'short>(
        &self,
        target: &'short mut Self::Target<'_>,
    ) -> Self::Target<'short> {
        // Target is &'a TestLock<T>.
        // &'short mut &'long TestLock<T> -> &'short TestLock<T>
        *target
    }

    unsafe fn access<'a>(&self, target: Self::Target<'a>) -> Self::OutputMut<'a> {
        // Safety: Caller guarantees lock is held.
        // target is &'a TestLock<T>.
        unsafe { &mut *target.contents.get() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_guard() {
        let lock = PureMutex::new(0);
        {
            let mut guard = lock.lock();
            assert_eq!(*guard, 0);
            *guard = 1;
        }

        {
            let guard = lock.lock();
            assert_eq!(*guard, 1);
        }
    }
}
