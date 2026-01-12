#![expect(unsafe_code, reason = "Required for GuardedOperation impl")]

use std::convert::Infallible;

use super::GuardedOperation;

/// A poisoned value. If a panic occurs while holding the lock, the lock is
/// poisoned and will return an error when acquired.
pub(crate) struct PoisonedValue<T> {
    poisoned: bool,
    value: T,
}

impl<T: std::panic::UnwindSafe> std::panic::UnwindSafe for PoisonedValue<T> {}

impl<T> PoisonedValue<T> {
    #[cfg_attr(not(test), expect(dead_code, reason = "experimental"))]
    pub(crate) fn new(value: T) -> Self {
        Self {
            poisoned: false,
            value,
        }
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct PoisonError;

impl From<Infallible> for PoisonError {
    fn from(value: Infallible) -> Self {
        match value {}
    }
}

/// A poisoned operation. If a panic occurs while holding the lock, the lock is
/// poisoned and will return an error when acquired.
pub(crate) struct PoisonOp<T> {
    // It is safe for a Poison guard to be Send and Sync, if T is Send and Sync
    _phantom: std::marker::PhantomData<T>,
}

impl<T> PoisonOp<T> {
    #[cfg_attr(not(test), expect(dead_code, reason = "experimental"))]
    pub(crate) fn new() -> Self {
        PoisonOp {
            _phantom: std::marker::PhantomData,
        }
    }
}

unsafe impl<T> GuardedOperation for PoisonOp<T> {
    type Target<'a>
        = &'a mut PoisonedValue<T>
    where
        Self: 'a;

    type SharedTarget<'a>
        = &'a PoisonedValue<T>
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
    type Error = PoisonError;

    fn acquire<'a>(&self, target: &mut Self::Target<'a>) -> Result<(), Self::Error>
    where
        Self: 'a,
    {
        if target.poisoned {
            return Err(PoisonError);
        }
        Ok(())
    }

    unsafe fn release<'a>(&self, target: &mut Self::Target<'a>)
    where
        Self: 'a,
    {
        assert!(!target.poisoned);
        if std::thread::panicking() {
            target.poisoned = true;
        }
    }

    unsafe fn target_to_shared<'short>(
        &self,
        target: &'short Self::Target<'_>,
    ) -> Self::SharedTarget<'short> {
        // target is &'short &'long mut PoisonedValue.
        // We want &'short T.
        target
    }

    fn view<'a>(&self, target: Self::SharedTarget<'a>) -> Self::Output<'a> {
        &target.value
    }

    unsafe fn reborrow<'short>(
        &self,
        target: &'short mut Self::Target<'_>,
    ) -> Self::Target<'short> {
        // Target is &'long mut PoisonedValue<T>
        // &'short mut &'long mut Pv -> &'short mut Pv
        // Use reborrowing
        *target
    }

    unsafe fn access<'a>(&self, target: Self::Target<'a>) -> Self::OutputMut<'a> {
        // target is &'a mut PoisonedValue<T>
        &mut target.value
    }
}

#[cfg(test)]
mod tests {

    use super::super::core::Guard;
    use super::*;

    #[test]
    fn test_non_poisoning() {
        let mut poisoned_value = PoisonedValue::new(0);

        std::thread::scope(|s| {
            s.spawn(|| {
                let mut guard = Guard::lock(PoisonOp::new(), &mut poisoned_value).unwrap();
                assert_eq!(*guard.contents(), 0);
                *guard.contents_mut() = 1;
            });
        });

        assert_eq!(poisoned_value.value, 1);

        assert!(Guard::lock(PoisonOp::new(), &mut poisoned_value).is_ok());
    }

    #[test]
    fn test_poisoning() {
        let mut poisoned_value = PoisonedValue::new(0);

        // This is not true in general, but it is in this case.
        let mut poisoned_mut_ref = std::panic::AssertUnwindSafe(&mut poisoned_value);

        std::thread::scope(|s| {
            s.spawn(move || {
                assert!(
                    std::panic::catch_unwind(move || {
                        let _guard = Guard::lock(PoisonOp::new(), &mut *poisoned_mut_ref).unwrap();
                        panic!("this is supposed to happen");
                    })
                    .is_err()
                );
            });
        });

        assert!(Guard::lock(PoisonOp::new(), &mut poisoned_value).is_err());
    }
}
