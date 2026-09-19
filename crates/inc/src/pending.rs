use std::{
    fmt::{Debug, Display},
    pin::Pin,
    sync::Arc,
};

use crate::{
    Ref,
    projected_arc::ProjArc,
    promise::{AnyPromise, Promise},
    types::AnyArc,
    universe::RefErr,
};

pub struct Pending<T> {
    /// The promise that reesolves into the base value. This is in the Any form,
    /// despite having a type variable because if T is Ref<V>, then this will
    /// be a Promise<V>, as we can convert the native internal arc to a Ref with
    /// no memory allocation.
    ///
    /// If this is any other type, the mapper will be set, and T will be the
    /// exact type expected.
    promise: AnyPromise,
    mapper: fn(&AnyArc) -> T,
}

fn plain_impl<T>(any_arc: &AnyArc) -> Ref<T>
where
    T: Send + Sync + 'static,
{
    let Ok(arc) = Arc::downcast::<T>(any_arc.clone()) else {
        // This is only used when the promise was originally a Promise<T>, so this
        // Arc is actually an Arc<T>.
        unreachable!("Enforced by Pending semantics.");
    };

    Ref::from_proj_arc(ProjArc::from_arc(arc))
}

fn can_fail_impl<T, E>(any_arc: &AnyArc) -> Result<Ref<T>, Ref<E>>
where
    T: Send + Sync + 'static,
    E: Send + Sync + 'static,
{
    let Ok(arc) = Arc::downcast::<Result<T, E>>(any_arc.clone()) else {
        // This is only used when the promise was originally a Promise<Result<T, E>>,
        unreachable!("Enforced by Pending semantics.");
    };

    Ref::from_arc(arc).can_fail()
}

fn can_fail_disp_impl<T, E>(any_arc: &AnyArc) -> Result<Ref<T>, RefErr<E>>
where
    T: Send + Sync + 'static,
    E: Display + Debug + Send + Sync + 'static,
{
    let Ok(arc) = Arc::downcast::<Result<T, E>>(any_arc.clone()) else {
        // This is only used when the promise was originally a Promise<Result<T, E>>,
        unreachable!("Enforced by Pending semantics.");
    };

    Ref::from_arc(arc).can_fail_disp()
}

impl<T> Pending<Ref<T>>
where
    T: Send + Sync + 'static,
{
    pub(crate) fn new_infallible(promise: Promise<T>) -> Self {
        Self {
            promise: promise.into_any(),
            mapper: plain_impl,
        }
    }
}

impl<T, E> Pending<Ref<Result<T, E>>>
where
    T: Send + Sync + 'static,
    E: Send + Sync + 'static,
{
    #[must_use]
    pub fn can_fail(self) -> Pending<Result<Ref<T>, Ref<E>>> {
        Pending {
            promise: self.promise,
            mapper: can_fail_impl,
        }
    }
}

impl<T, E> Pending<Ref<Result<T, E>>>
where
    T: Send + Sync + 'static,
    E: Display + Debug + Send + Sync + 'static,
{
    #[must_use]
    pub fn can_fail_disp(self) -> Pending<Result<Ref<T>, RefErr<E>>> {
        Pending {
            promise: self.promise,
            mapper: can_fail_disp_impl,
        }
    }
}

impl<T> Pending<T>
where
    T: Send + Sync + 'static,
{
    #[must_use]
    pub fn get_sync(&self) -> T {
        (self.mapper)(self.promise.get_sync_arc())
    }

    pub async fn get_async(&self) -> T {
        (self.mapper)(self.promise.get_async_arc().await)
    }
}

impl<T> IntoFuture for Pending<T>
where
    T: Send + Sync + 'static,
{
    type Output = T;
    type IntoFuture = Pin<Box<dyn Future<Output = T> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move { self.get_async().await })
    }
}
