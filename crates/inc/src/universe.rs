use std::{
    any::Any,
    hash::Hash,
    ops::Deref,
    sync::{Arc, Mutex},
};

use crate::{
    AsyncOperation, artifact_store::ArtifactStore, executor::Executor, operations::Operation,
    type_map::TypeMap,
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

    pub fn get_sync<O>(&self, op: O) -> Handle<O::Value>
    where
        O: Operation,
    {
        let ctx = TaskContext::new(&self.inner);
        ctx.get_sync(op)
    }

    pub async fn get_async<O>(&self, op: O) -> Handle<O::Value>
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

pub struct Handle<T>(Arc<T>);

impl<T> Handle<T> {
    #[must_use]
    pub fn get(&self) -> &T {
        &self.0
    }
}

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Deref for Handle<T> {
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
    pub fn try_get_input<I>(&self) -> Option<Handle<I::Value>>
    where
        I: Input,
    {
        let guard = self.inner.inputs.lock().unwrap();
        guard
            .get::<I>()
            .map(|v| Handle(Arc::downcast(v.clone()).expect("Downcast to I")))
    }

    #[must_use]
    pub fn get_input<I>(&self) -> Handle<I::Value>
    where
        I: Input,
    {
        self.try_get_input::<I>().expect("Input not found")
    }

    pub fn get_sync<O>(&self, op: O) -> Handle<O::Value>
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
        Handle(output)
    }

    pub async fn get_async<O>(&self, op: O) -> Handle<O::Value>
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
        Handle(output)
    }
}

pub trait Artifact: Send + Sync + 'static {}
impl<T> Artifact for T where T: Send + Sync + 'static {}

pub trait ArtifactKey: Artifact + Eq + Hash {
    type Value: Artifact;
}

pub trait Input: ArtifactKey {}
