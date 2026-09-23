use std::{
    fmt::{Debug, Display},
    sync::Arc,
};

use crate::{
    Ref,
    projected_arc::ProjArc,
    promise::{SharedAnyPromise, SharedPromise},
    types::AnyArc,
    universe::RefErr,
};

pub struct Pending<'a, T> {
    /// The promise that reesolves into the base value. This is in the Any form,
    /// despite having a type variable because if T is Ref<V>, then this will
    /// be a Promise<V>, as we can convert the native internal arc to a Ref with
    /// no memory allocation.
    ///
    /// If this is any other type, the mapper will be set, and T will be the
    /// exact type expected.
    promise: &'a SharedAnyPromise,
    mapper: fn(&'a AnyArc) -> T,
}

fn plain_impl<T>(any: &AnyArc) -> &T
where
    T: Send + Sync + 'static,
{
    let Some(v) = any.downcast_ref() else {
        unreachable!("Enforced by Pending semantics.");
    };
    v
}

fn can_fail_impl<T, E>(any: &AnyArc) -> Result<&T, &E>
where
    T: Send + Sync + 'static,
    E: Send + Sync + 'static,
{
    let Some(result) = any.downcast_ref::<Result<T, E>>() else {
        // This is only used when the promise was originally a Promise<Result<T, E>>,
        unreachable!("Enforced by Pending semantics.");
    };

    result.as_ref()
}

fn can_fail_disp_impl<T, E>(any_arc: &AnyArc) -> Result<&T, RefErr<E>>
where
    T: Send + Sync + 'static,
    E: Display + Debug + Send + Sync + 'static,
{
    let Some(result) = any_arc.downcast_ref::<Result<T, E>>() else {
        unreachable!("Enforced by Pending semantics");
    };

    if let Ok(v_ref) = result {
        return Ok(v_ref);
    }

    let Ok(result) = Arc::downcast::<Result<T, E>>(any_arc.clone()) else {
        unreachable!("Enforced by pending semantics.");
    };

    let proj_arc = ProjArc::from_arc(result);

    Err(RefErr::from_ref(Ref::from_proj_arc(ProjArc::map(
        &proj_arc,
        |r| r.as_ref().err().unwrap(),
    ))))
}

impl<'a, T> Pending<'a, &'a T>
where
    T: Send + Sync + 'static,
{
    pub(crate) fn new_infallible(promise: &'a SharedPromise<T>) -> Self {
        Self {
            promise: promise.as_any(),
            mapper: plain_impl,
        }
    }
}

impl<'a, T, E> Pending<'a, &'a Result<T, E>>
where
    T: Send + Sync + 'static,
    E: Send + Sync + 'static,
{
    #[must_use]
    pub fn can_fail(self) -> Pending<'a, Result<&'a T, &'a E>> {
        Pending {
            promise: self.promise,
            mapper: can_fail_impl,
        }
    }
}

impl<'a, T, E> Pending<'a, &'a Result<T, E>>
where
    T: Send + Sync + 'static,
    E: Display + Debug + Send + Sync + 'static,
{
    #[must_use]
    pub fn can_fail_disp(self) -> Pending<'a, Result<&'a T, RefErr<E>>> {
        Pending {
            promise: self.promise,
            mapper: can_fail_disp_impl,
        }
    }
}

impl<'a, T> Pending<'a, T>
where
    T: Send + Sync + 'a,
{
    #[must_use]
    pub fn get_sync(&self) -> T {
        (self.mapper)(self.promise.get_sync_arc())
    }

    pub async fn get_async(&self) -> T {
        (self.mapper)(self.promise.get_async_arc().await)
    }
}
