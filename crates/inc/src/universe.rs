use std::{
    any::Any,
    hash::Hash,
    ops::Deref,
    pin::Pin,
    sync::{Arc, Mutex},
};

use crate::{
    AsyncOperation,
    artifact_store::ArtifactStore,
    executor::Executor,
    operations::Operation,
    projected_arc::ProjArc,
    promise::{AnyPromise, Promise},
    type_map::TypeMap,
    types::AnyArc,
};

struct Inner {
    inputs: Mutex<TypeMap<Arc<dyn Any + Send + Sync + 'static>>>,
    artifacts: ArtifactStore,
    executor: Executor,
}

pub struct Universe {
    inner: Arc<Inner>,
}

impl Universe {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                inputs: Mutex::new(TypeMap::new()),
                artifacts: ArtifactStore::new(),
                executor: Executor::new_simple(),
            }),
        }
    }

    pub fn add_input<I: Input>(&mut self, input: I::Value) -> &mut Self {
        self.inner
            .inputs
            .lock()
            .unwrap()
            .unique_insert::<I>(Arc::new(input))
            .ok()
            .unwrap();
        self
    }

    pub fn get_sync<O>(&self, op: O) -> Ref<O::Value>
    where
        O: Operation,
    {
        let ctx = TaskContext::new(&self.inner);
        ctx.get_sync(op)
    }

    pub async fn get_async<O>(&self, op: O) -> Ref<O::Value>
    where
        O: AsyncOperation,
    {
        let ctx = TaskContext::new(&self.inner);
        ctx.get_async(op).await
    }
}

impl Default for Universe {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Ref<T>(ProjArc<T>);

impl<T> Ref<T> {
    #[must_use]
    pub fn get(&self) -> &T {
        &self.0
    }
}

impl<T> Clone for Ref<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Deref for Ref<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A context passed to each operation to interact with the Universe.
///
/// This permits trying to obtain new operations within another operation.
pub struct TaskContext<'a> {
    inner: &'a Arc<Inner>,
}

impl<'a> TaskContext<'a> {
    fn new(inner: &'a Arc<Inner>) -> Self {
        Self { inner }
    }

    #[must_use]
    pub fn try_get_input<I>(&self) -> Option<Ref<I::Value>>
    where
        I: Input,
    {
        let guard = self.inner.inputs.lock().unwrap();
        guard.get::<I>().map(|v| {
            Ref(ProjArc::from_arc(
                Arc::downcast(v.clone()).expect("Downcast to I"),
            ))
        })
    }

    #[must_use]
    pub fn get_input<I>(&self) -> Ref<I::Value>
    where
        I: Input,
    {
        self.try_get_input::<I>().expect("Input not found")
    }

    pub fn get_sync<O>(&self, op: O) -> Ref<O::Value>
    where
        O: Operation + 'a,
    {
        let op = Arc::new(op);
        let output = self
            .inner
            .artifacts
            .get_or_run_sync(&self.inner.executor, op.clone(), {
                let inner = self.inner.clone();
                move || op.run(&TaskContext::new(&inner))
            });
        Ref(ProjArc::from_arc(output))
    }

    pub async fn get_async<O>(&self, op: O) -> Ref<O::Value>
    where
        O: AsyncOperation,
    {
        let op = Arc::new(op);
        let output = self
            .inner
            .artifacts
            .get_or_run_async(&self.inner.executor, op.clone(), {
                let inner = self.inner.clone();
                async move { op.run(&TaskContext::new(&inner)).await }
            })
            .await;
        Ref(ProjArc::from_arc(output))
    }
}

pub trait Artifact: Send + Sync + 'static {}
impl<T> Artifact for T where T: Send + Sync + 'static {}

pub trait ArtifactKey: Artifact + Eq + Hash {
    type Value: Artifact;
}

pub trait Input: ArtifactKey {}

pub(crate) struct Pending<T> {
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

fn extract_ref_from_any_arc<T>(any_arc: &AnyArc) -> Ref<T>
where
    T: Send + Sync + 'static,
{
    let Ok(arc) = Arc::downcast::<T>(any_arc.clone()) else {
        // This is only used when the promise was originally a Promise<T>, so this
        // Arc is actually an Arc<T>.
        unreachable!("Enforced by Pending semantics.");
    };

    Ref(ProjArc::from_arc(arc))
}

fn extract_result_from_any_arc<T, E>(any_arc: &AnyArc) -> Result<Ref<T>, Ref<E>>
where
    T: Send + Sync + 'static,
    E: Send + Sync + 'static,
{
    let Ok(arc) = Arc::downcast::<Result<T, E>>(any_arc.clone()) else {
        // This is only used when the promise was originally a Promise<Result<T, E>>,
        unreachable!("Enforced by Pending semantics.");
    };

    let proj_arc = ProjArc::from_arc(arc);
    match proj_arc.as_ref() {
        Ok(_) => Ok(Ref(ProjArc::map(&proj_arc, |v| v.as_ref().ok().unwrap()))),
        Err(_) => Err(Ref(ProjArc::map(&proj_arc, |v| v.as_ref().err().unwrap()))),
    }
}

impl<T> Pending<Ref<T>>
where
    T: Send + Sync + 'static,
{
    fn new_infallible(promise: Promise<T>) -> Self {
        Self {
            promise: promise.into_any(),
            mapper: extract_ref_from_any_arc,
        }
    }
}

impl<T, E> Pending<Result<Ref<T>, Ref<E>>>
where
    T: Send + Sync + 'static,
    E: Send + Sync + 'static,
{
    fn new_fallible(promise: Promise<Result<T, E>>) -> Self {
        Self {
            promise: promise.into_any(),
            mapper: extract_result_from_any_arc,
        }
    }
}

impl<T> Pending<T>
where
    T: Send + Sync + 'static,
{
    pub(crate) fn get_sync(&self) -> T {
        (self.mapper)(self.promise.get_sync_arc())
    }

    pub(crate) async fn get_async(&self) -> T {
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
