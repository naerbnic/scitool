#![expect(unsafe_code, reason = "necessary for GuardedOperation")]
use super::core::GuardedOperation;

/// Composes two operations into a single operation, where the second operation
/// takes the first operation's output as its target.
pub(crate) struct ComposeOps<Op1, Op2, E>
where
    Op1: GuardedOperation,
    Op2: GuardedOperation,
{
    op1: Op1,
    op2: Op2,
    _phantom: std::marker::PhantomData<fn() -> E>,
}

impl<Op1, Op2, E> ComposeOps<Op1, Op2, E>
where
    Op1: GuardedOperation,
    Op2: GuardedOperation,
    // Op2 target is Op1 output mut
    for<'a> Op2: GuardedOperation<Target<'a> = Op1::OutputMut<'a>>,
    // Op2 shared target is Op1 output
    for<'a> Op2: GuardedOperation<SharedTarget<'a> = Op1::Output<'a>>,
    E: From<Op1::Error> + From<Op2::Error> + 'static,
{
    /// Creates a new composed operation.
    #[cfg_attr(not(test), expect(dead_code, reason = "experimental"))]
    pub(super) fn new(op1: Op1, op2: Op2) -> Self {
        Self {
            op1,
            op2,
            _phantom: std::marker::PhantomData,
        }
    }
}

unsafe impl<Op1, Op2, E> GuardedOperation for ComposeOps<Op1, Op2, E>
where
    Op1: GuardedOperation,
    Op2: GuardedOperation,
    // Op2 target is Op1 output mut
    for<'a> Op2: GuardedOperation<Target<'a> = Op1::OutputMut<'a>> + 'a,
    // Op2 shared target is Op1 output
    for<'a> Op2: GuardedOperation<SharedTarget<'a> = Op1::Output<'a>>,
    E: From<Op1::Error> + From<Op2::Error> + 'static,
{
    type Target<'a>
        = Op1::Target<'a>
    where
        Op1: 'a,
        Op2: 'a;

    type SharedTarget<'a>
        = Op2::SharedTarget<'a>
    where
        Op1: 'a,
        Op2: 'a;

    type Output<'a>
        = Op2::Output<'a>
    where
        Op1: 'a,
        Op2: 'a;
    type OutputMut<'a>
        = Op2::OutputMut<'a>
    where
        Op1: 'a,
        Op2: 'a;
    type Error = E;

    fn acquire<'a>(&self, target: &mut Self::Target<'a>) -> Result<(), Self::Error>
    where
        Self: 'a,
    {
        self.op1.acquire(target)?;

        let result = {
            // Safety: op1 is acquired. We reborrow target to pass to access.
            let target_op1 = unsafe { self.op1.reborrow(target) };
            let target2 = unsafe { self.op1.access(target_op1) };
            self.op2.acquire(&mut safe_cast(target2))
        };

        match result {
            Ok(()) => Ok(()),
            Err(e) => {
                // Rollback op1
                unsafe { self.op1.release(target) };
                Err(e.into())
            }
        }
    }

    fn try_acquire<'a>(&self, target: &mut Self::Target<'a>) -> Result<bool, Self::Error>
    where
        Self: 'a,
    {
        if !self.op1.try_acquire(target)? {
            return Ok(false);
        }

        let result = {
            let target_op1 = unsafe { self.op1.reborrow(target) };
            let target2 = unsafe { self.op1.access(target_op1) };
            self.op2.try_acquire(&mut safe_cast(target2))
        };

        match result {
            Ok(true) => Ok(true),
            Ok(false) => {
                // Rollback op1
                unsafe { self.op1.release(target) };
                Ok(false)
            }
            Err(e) => {
                // Rollback op1
                unsafe { self.op1.release(target) };
                Err(e.into())
            }
        }
    }

    unsafe fn release<'a>(&self, target: &mut Self::Target<'a>)
    where
        Self: 'a,
    {
        {
            // Safety: Caller ensures op1 is acquired.
            let target_op1 = unsafe { self.op1.reborrow(target) };
            let target2 = unsafe { self.op1.access(target_op1) };
            // Ensure temporary is dropped
            let mut t2 = safe_cast(target2);
            unsafe {
                self.op2.release(&mut t2);
            }
        }
        unsafe {
            self.op1.release(target);
        }
    }

    unsafe fn target_to_shared<'short>(
        &self,
        target: &'short Self::Target<'_>,
    ) -> Self::SharedTarget<'short> {
        // Safety: Caller ensures lock is acquired.
        let shared1 = unsafe { self.op1.target_to_shared(target) };
        let shared2 = self.op1.view(shared1);
        safe_cast(shared2)
    }

    fn view<'a>(&self, target: Self::SharedTarget<'a>) -> Self::Output<'a> {
        self.op2.view(target)
    }

    unsafe fn reborrow<'short>(
        &self,
        target: &'short mut Self::Target<'_>,
    ) -> Self::Target<'short> {
        unsafe { self.op1.reborrow(target) }
    }

    unsafe fn access<'a>(&self, target: Self::Target<'a>) -> Self::OutputMut<'a> {
        // Safety: Caller ensures acquired.
        let target2 = unsafe { self.op1.access(target) };
        // Safety: target2 corresponds to scope of target borrow. op2 acquired.
        unsafe { self.op2.access(safe_cast(target2)) }
    }
}

fn safe_cast<T, U>(val: T) -> U
where
    T: Into<U>,
{
    val.into()
}

#[cfg(test)]
mod tests {
    use super::super::{
        core::Guard,
        mutex::{PureMutex, PureMutexOp},
        poison::{PoisonError, PoisonOp, PoisonedValue},
    };
    use super::*;
    #[test]
    fn test_compose_ops() {
        type PoisonedLockOp<T> =
            ComposeOps<PureMutexOp<PoisonedValue<T>>, PoisonOp<T>, PoisonError>;
        let lock = PureMutex::new(PoisonedValue::new(10));

        // Test normal access
        {
            let mut guard = Guard::lock(
                PoisonedLockOp::new(PureMutexOp::new(), PoisonOp::new()),
                &lock,
            )
            .unwrap();
            assert_eq!(*guard.contents(), 10);
            *guard.contents_mut() = 20;
        }

        // Verify value updated
        {
            let guard = Guard::lock(
                PoisonedLockOp::new(PureMutexOp::new(), PoisonOp::new()),
                &lock,
            )
            .unwrap();
            assert_eq!(*guard.contents(), 20);
        }

        // Test poisoning
        let thread_res = std::thread::spawn({
            // We need a thread-safe reference to pass to the thread.
            // PureMutex is Sync if T is Send. PoisonedValue<i32> is Send.
            // We need to use Arc mostly because PureMutex isn't cloneable, ref is.
            // But simpler: just use statics or scoped threads if available, but std::thread is fine.
            // Actually, we can't easily pass &PureMutex to thread without scope.
            // Let's rely on panicking behavior being thread local if we catch unwind?
            // But release checks thread::panicking().
            // Use Arc.
            let lock_arc = std::sync::Arc::new(PureMutex::new(PoisonedValue::new(0)));
            let lock_arc_clone = lock_arc.clone();
            move || {
                let _temp = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _guard = Guard::lock(
                        PoisonedLockOp::new(PureMutexOp::new(), PoisonOp::new()),
                        &*lock_arc_clone,
                    )
                    .unwrap();
                    panic!("oops");
                }));
                lock_arc
            }
        })
        .join()
        .unwrap();

        // Now try to lock again
        let res = Guard::lock(
            PoisonedLockOp::new(PureMutexOp::new(), PoisonOp::new()),
            &*thread_res,
        );
        match res {
            Err(e) => assert_eq!(e, PoisonError),
            Ok(_) => panic!("Should have been poisoned"),
        }
    }
}
