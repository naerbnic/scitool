#![expect(unsafe_code)]

/// A trait representing an operation that can be used to acquire and release
/// some resource on a target value.
///
/// The [`Send`]/[`Sync`]-ability of the target type determines the [`Send`]/[`Sync`]
/// -ability of the [`Guard`].
///
/// # Safety
///
/// As a general comment, there are several operations on this trait that are
/// only valid when the resource is acquired. This means, for a single instance
/// of the [`Self`] type, the following must be true:
///
/// - Either [`GuardedOperation::acquire()`] or [`GuardedOperation::try_acquire()`]
///   must have been called successfully. In the case of [`try_acquire()`], it
///   must have returned `Ok(true)`.
/// - [`GuardedOperation::release()`] must not have been called.
///
/// We call this the "guarded state". Note that only counts for a single instance
/// of the operation. It is possible for there to be multiple operation values
/// in the logical guarded state, such as read-only locks.
///
/// Other requirements:
///
/// - The following traits must be accurate to represent the behavior of its
///   guard's implementation:
///   - [`Send`]: if the Guard can be sent to another thread
///   - [`Sync`]: if the Guard can be shared between threads
///   - [`Clone`]: if the Guard can be cloned
/// - After a call to [`GuardedOperation::acquire()`] or [`GuardedOperation::try_acquire()`],
///   all calls to other methods that take a target must refer to the target, including:
///   - [`GuardedOperation::target_to_shared()`]
///   - [`GuardedOperation::reborrow()`]
///   - [`GuardedOperation::access()`]
///   - [`GuardedOperation::release()`]
/// - All methods that take a [`GuardedOperation::SharedTarget`] must obtain their value
///   from a call to [`GuardedOperation::target_to_shared()`].
pub(crate) unsafe trait GuardedOperation {
    /// The mutable target type. This is the type that is passed to acquire and
    /// release.
    type Target<'a>
    where
        Self: 'a;

    /// A shared version of the target type. This must be reborrowable from the
    /// mutable target type.
    type SharedTarget<'a>
    where
        Self: 'a;

    /// The output value that can be returned from `view` and `access`. while
    /// the guard is held.
    type Output<'a>
    where
        Self: 'a;

    /// The output value that can be returned from `access` while the guard is
    /// held.
    type OutputMut<'a>
    where
        Self: 'a;

    /// An error type that can be returned from `acquire` and `try_acquire`
    type Error;

    /// Takes a lock on the value, providing a guard as evidence that the lock
    /// is held.
    ///
    /// It is a contract error to call this function with self again if it has
    /// already been called on the same instance of the [`Self`] type, and not
    /// released.
    ///
    /// # Post-conditions
    ///
    /// - If this method returns `Ok(())`, the target effectively acquired.
    /// - If this method returns `Err(_)`, the target is **not** acquired.
    ///   It must be in the same state as when it was passed to `acquire`.
    fn acquire<'a>(&self, target: &mut Self::Target<'a>) -> Result<(), Self::Error>
    where
        Self: 'a;

    /// Tries to take a lock on the value, with the option of denying it if
    /// there is already a state that indicates the value is unavailable, but
    /// not an error.
    ///
    /// It is a contract error to call this function with self again if it has
    /// already been called on the same instance of the [`Self`] type, and not
    /// released.
    ///
    /// # Post-conditions
    ///
    /// - If this method returns `Ok(true)`, the target effectively acquired.
    /// - If this method returns `Ok(false)` or `Err(_)`, the target is **not**
    ///   acquired. It must be in the same state as when it was passed to
    ///   `try_acquire`.
    fn try_acquire<'a>(&self, target: &mut Self::Target<'a>) -> Result<bool, Self::Error>
    where
        Self: 'a,
    {
        self.acquire(target)?;
        Ok(true)
    }

    /// Releases the guard on the value.
    ///
    /// This may be called during a drop implementation, so we may be in a panic
    /// state.
    ///
    /// # Safety
    ///
    /// - The caller must ensure that `acquire()` or `try_acquire()` has been called
    ///   on the target value with the same operation value, and `release()` has not
    ///   been called yet. Calls to `acquire()` may be nested, if allowed by the
    ///   implementation.
    /// - Any output value from `get_contents()` must have ben dropped before
    ///   `release()` is called.
    unsafe fn release<'a>(&self, target: &mut Self::Target<'a>)
    where
        Self: 'a;

    /// Converts the target to a shared target reference, which serves as a
    /// proof that the lock is held (and thus shared access is safe).
    ///
    /// # Safety
    ///
    /// - The operation and target must be in the "guarded state".
    unsafe fn target_to_shared<'short>(
        &self,
        target: &'short Self::Target<'_>,
    ) -> Self::SharedTarget<'short>;

    /// Gets a view of the contents of the value.
    fn view<'a>(&self, target: Self::SharedTarget<'a>) -> Self::Output<'a>;

    /// Creates a shorter-lived target handle from a borrowed longer-lived handle.
    ///
    /// This allows us to "downgrade" the lifetime of a mutable borrow of a target
    /// (which we might hold in the Guard) to a shorter lifetime that can be
    /// consumed by `access`.
    ///
    /// # Safety
    ///
    /// - `target` must be a mutable reference to the same logical target as was
    ///   acquired
    unsafe fn reborrow<'short>(&self, target: &'short mut Self::Target<'_>)
    -> Self::Target<'short>;

    /// Gets mutability access to the contents of the value.
    ///
    /// # Safety
    ///
    /// - The operation and target must be in the "guarded state".
    /// - `target` must be a mutable reference to the same logical target as was
    ///   acquired
    /// - The lifetime of the returned `OutputMut` must not overlap with a
    ///   separate call to `access` or `release` on this operation.
    unsafe fn access<'a>(&self, target: Self::Target<'a>) -> Self::OutputMut<'a>;
}

mod guard_contents {
    use super::GuardedOperation;
    use std::cell::UnsafeCell;

    /// A type that wraps a [`UnsafeCell`], but is send/sync.
    ///
    /// This is not generally safe, but is needed in [`Guard`] to ensure that the
    /// [`Send`]/[`Sync`] bounds are based on the Op type, and not exclusively
    /// blocked by the [`UnsafeCell`].
    pub(super) struct GuardContents<'a, Op>(UnsafeCell<Op::Target<'a>>)
    where
        Op: GuardedOperation + 'a;

    impl<'a, Op> GuardContents<'a, Op>
    where
        Op: GuardedOperation + 'a,
    {
        /// Creates a new `GuardContents` wrapper.
        ///
        /// This method is safe because:
        /// 1. `UnsafeCell::new` is safe.
        /// 2. The unsafe `Send` and `Sync` implementations on this struct are
        ///    strictly bounded by the `Send` and `Sync` implementation of the
        ///    inner `Op::Target` type.
        pub(super) fn new(target: Op::Target<'a>) -> Self {
            Self(UnsafeCell::new(target))
        }

        pub(super) fn get(&self) -> &Op::Target<'a> {
            unsafe { &*self.0.get() }
        }

        pub(super) fn get_raw(&self) -> *mut Op::Target<'a> {
            self.0.get()
        }
    }

    impl<'a, Op> Clone for GuardContents<'a, Op>
    where
        Op: GuardedOperation + 'a,
        Op::Target<'a>: Clone,
    {
        fn clone(&self) -> Self {
            Self(UnsafeCell::new(Clone::clone(unsafe { &*self.0.get() })))
        }
    }

    unsafe impl<'a, Op> Send for GuardContents<'a, Op>
    where
        Op: GuardedOperation + 'a,
        Op::Target<'a>: Send,
    {
    }

    unsafe impl<'a, Op> Sync for GuardContents<'a, Op>
    where
        Op: GuardedOperation + 'a,
        Op::Target<'a>: Sync,
    {
    }
}

use self::guard_contents::GuardContents;

/// A guard that provides access to a protected value, ensuring that the necessary
/// locks or resources are held for the duration of the guard's lifetime.
///
/// This struct uses RAII to automatically release the operation/lock when dropped.
pub(crate) struct Guard<'a, Op>
where
    Op: GuardedOperation + 'a,
{
    op: Op,
    target: GuardContents<'a, Op>,
}

impl<'a, Op> Guard<'a, Op>
where
    Op: GuardedOperation + 'a,
{
    /// Acquires the lock via the provided operation and constructs a `Guard`.
    ///
    /// This method blocks until the lock is acquired.
    pub(crate) fn lock(op: Op, mut target: Op::Target<'a>) -> Result<Self, Op::Error> {
        op.acquire(&mut target)?;
        // Safety: The GuardedOperation guarantees that the target is safe to access
        // for the same sync/send bounds as the Op type.
        let target = GuardContents::new(target);
        Ok(Self { op, target })
    }

    /// Attempts to acquire the lock via the provided operation.
    ///
    /// Returns:
    /// - `Ok(Some(Guard))` if the lock was successfully acquired.
    /// - `Ok(None)` if the lock could not be acquired immediately (but no error occurred).
    /// - `Err(e)` if a strict error occurred.
    #[cfg_attr(not(test), expect(dead_code, reason = "experimental"))]
    #[cfg_attr(test, expect(dead_code, reason = "not_tested"))]
    pub(crate) fn try_lock(op: Op, mut target: Op::Target<'a>) -> Result<Option<Self>, Op::Error> {
        let succeeded = op.try_acquire(&mut target)?;
        if !succeeded {
            return Ok(None);
        }
        // Safety: The GuardedOperation guarantees that the target is safe to access
        // for the same sync/send bounds as the Op type.
        let target = GuardContents::new(target);
        Ok(Some(Self { op, target }))
    }

    /// Returns a shared view of the protected contents.
    pub(crate) fn contents(&self) -> Op::Output<'_> {
        // SAFETY: We hold the guard, so the lock is held. We can safely convert
        // the target to a shared target.
        unsafe {
            let target_ref = self.target.get();
            let shared = self.op.target_to_shared(target_ref);
            self.op.view(shared)
        }
    }

    /// Returns a mutable view of the protected contents.
    pub(crate) fn contents_mut(&mut self) -> Op::OutputMut<'_> {
        // SAFETY: We hold the guard, so the lock is held. We have &mut self,
        // so we have exclusive access.
        unsafe {
            let target_long = &mut *self.target.get_raw();
            // We reborrow it to the lifetime of the call to contents_mut
            let target_short = self.op.reborrow(target_long);
            self.op.access(target_short)
        }
    }
}

impl<Op> Drop for Guard<'_, Op>
where
    Op: GuardedOperation,
{
    fn drop(&mut self) {
        unsafe { self.op.release(&mut *self.target.get_raw()) }
    }
}
