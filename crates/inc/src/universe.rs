use std::{
    any::Any,
    hash::Hash,
    ops::Deref,
    sync::{Arc, Mutex},
};

use crate::{
    AsyncOperation, artifact_store::ArtifactStore, executor::Executor, operations::Operation,
    pending::Pending, projected_arc::ProjArc, type_map::TypeMap, untyped_key::UntypedKey,
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

    pub fn sync_op<O>(&self, op: O) -> Pending<Ref<O::Value>>
    where
        O: Operation,
    {
        let ctx = TaskContext::new(&self.inner);
        ctx.sync_op(op)
    }
}

impl Default for Universe {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Ref<T>(ProjArc<T>);

impl<T> Ref<T> {
    pub(crate) fn from_proj_arc(proj_arc: ProjArc<T>) -> Self {
        Self(proj_arc)
    }

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

    pub fn sync_op<O>(&self, op: O) -> Pending<Ref<O::Value>>
    where
        O: Operation + 'a,
    {
        let op = Arc::new(op);
        let output = self.inner.artifacts.enqueue_sync(
            &self.inner.executor,
            &UntypedKey::from_arc(op.clone()),
            {
                let inner = self.inner.clone();
                move || op.run(&TaskContext::new(&inner))
            },
        );
        Pending::new_infallible(output)
    }
}

pub trait Artifact: Send + Sync + 'static {}
impl<T> Artifact for T where T: Send + Sync + 'static {}

pub trait ArtifactKey: Artifact + Eq + Hash {
    type Value: Artifact;
}

pub trait Input: ArtifactKey {}
