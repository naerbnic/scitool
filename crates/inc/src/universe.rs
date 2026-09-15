use std::{
    any::Any,
    collections::HashMap,
    ops::Deref,
    sync::{Arc, Mutex},
};

use crate::{
    AsyncOperation, executor::Executor, operations::Operation, type_map::TypeMap,
    untyped_key::UntypedKey,
};

struct Inner {
    inputs: Mutex<TypeMap<Arc<dyn Any + Send + Sync + 'static>>>,
    artifacts: Mutex<HashMap<UntypedKey, Arc<dyn Any + Send + Sync + 'static>>>,
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
                artifacts: Mutex::new(HashMap::new()),
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

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Handle<T> {
    #[must_use]
    pub fn get(&self) -> &T {
        &self.0
    }
}

impl<T> Deref for Handle<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

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
        let new_key = UntypedKey::from_arc(op.clone());
        if let Some(output) = self.inner.artifacts.lock().unwrap().get(&new_key) {
            return Handle(output.clone().downcast::<O::Value>().unwrap());
        }

        let (trigger, waiter) = crate::event::new_event_pair();

        self.inner.executor.schedule({
            let inner = self.inner.clone();
            let new_key = new_key.clone();
            move || {
                let output = op.run(&TaskContext::new(&inner));
                inner
                    .artifacts
                    .lock()
                    .unwrap()
                    .insert(new_key, Arc::new(output));
                trigger.trigger();
            }
        });

        waiter.wait_sync().unwrap();
        let guard = self.inner.artifacts.lock().unwrap();
        let Some(output) = guard.get(&new_key) else {
            unreachable!("event trigger indicates completion");
        };
        Handle(output.clone().downcast::<O::Value>().unwrap())
    }

    pub async fn get_async<O>(&self, op: O) -> Handle<O::Value>
    where
        O: AsyncOperation,
    {
        let op = Arc::new(op);
        let new_key = UntypedKey::from_arc(op.clone());
        if let Some(output) = self.inner.artifacts.lock().unwrap().get(&new_key) {
            return Handle(output.clone().downcast::<O::Value>().unwrap());
        }

        let (trigger, waiter) = crate::event::new_event_pair();

        self.inner.executor.schedule_async({
            let inner = self.inner.clone();
            let new_key = new_key.clone();
            async move {
                let output = op.run(&TaskContext::new(&inner)).await;
                inner
                    .artifacts
                    .lock()
                    .unwrap()
                    .insert(new_key, Arc::new(output));
                trigger.trigger();
            }
        });

        waiter.wait_async().await.unwrap();
        let guard = self.inner.artifacts.lock().unwrap();
        let Some(output) = guard.get(&new_key) else {
            unreachable!("event trigger indicates completion");
        };
        Handle(output.clone().downcast::<O::Value>().unwrap())
    }
}

pub trait Artifact: Send + Sync + 'static {}

pub trait ArtifactKey: Artifact {
    type Value: Artifact;
}

pub trait Input: ArtifactKey {}

impl<T> Artifact for T where T: Send + Sync + 'static {}
