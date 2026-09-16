use std::{
    any::Any,
    collections::{HashMap, hash_map},
    sync::{Arc, Mutex},
};

use crate::{
    Artifact, ArtifactKey, event::EventWaiter, executor::Executor, untyped_key::UntypedKey,
};

type AnyArc = Arc<dyn Any + Send + Sync>;

#[derive(Clone)]
enum ArtifactState {
    Pending(EventWaiter),
    Resolved(AnyArc),
}

pub(crate) struct ArtifactStore {
    artifacts: Arc<Mutex<HashMap<UntypedKey, ArtifactState>>>,
}

impl ArtifactStore {
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            artifacts: Arc::default(),
        }
    }

    fn enqueue_sync<T, F>(&self, executor: &Executor, key: &UntypedKey, body: F) -> ArtifactState
    where
        T: Artifact,
        F: FnOnce() -> T + Send + 'static,
    {
        let mut guard = self.artifacts.lock().unwrap();
        match guard.entry(key.clone()) {
            hash_map::Entry::Occupied(entry) => entry.get().clone(),
            hash_map::Entry::Vacant(vacant_entry) => {
                let (mut trigger, waiter) = crate::event::new_event_pair();
                executor.schedule_async({
                    let artifacts = self.artifacts.clone();
                    let key = key.clone();
                    async move {
                        let value = body();
                        let mut guard = artifacts.lock().unwrap();
                        guard.insert(key, ArtifactState::Resolved(Arc::new(value)));
                        trigger.trigger();
                    }
                });
                vacant_entry.insert(ArtifactState::Pending(waiter)).clone()
            }
        }
    }

    #[must_use]
    pub(crate) fn get_or_run_sync<K: ArtifactKey, F>(
        &self,
        executor: &Executor,
        key: Arc<K>,
        body: F,
    ) -> Arc<K::Value>
    where
        F: FnOnce() -> K::Value + Send + 'static,
    {
        let key = UntypedKey::from_arc(key);

        match self.enqueue_sync(executor, &key, body) {
            ArtifactState::Resolved(value) => Arc::downcast(value).unwrap(),
            ArtifactState::Pending(waiter) => {
                waiter.wait_sync().unwrap();
                let guard = self.artifacts.lock().unwrap();
                let Some(ArtifactState::Resolved(value)) = guard.get(&key) else {
                    unreachable!("triggered only when resolved");
                };

                Arc::downcast(value.clone()).unwrap()
            }
        }
    }

    fn enqueue_async<T, Fut>(
        &self,
        executor: &Executor,
        key: &UntypedKey,
        body: Fut,
    ) -> ArtifactState
    where
        T: Artifact,
        Fut: Future<Output = T> + Send + 'static,
    {
        let mut guard = self.artifacts.lock().unwrap();
        match guard.entry(key.clone()) {
            hash_map::Entry::Occupied(entry) => entry.get().clone(),
            hash_map::Entry::Vacant(vacant_entry) => {
                let (mut trigger, waiter) = crate::event::new_event_pair();
                executor.schedule_async({
                    let artifacts = self.artifacts.clone();
                    let key = key.clone();
                    async move {
                        let value = body.await;
                        let mut guard = artifacts.lock().unwrap();
                        guard.insert(key, ArtifactState::Resolved(Arc::new(value)));
                        trigger.trigger();
                    }
                });
                vacant_entry.insert(ArtifactState::Pending(waiter)).clone()
            }
        }
    }

    #[must_use]
    pub(crate) async fn get_or_run_async<K: ArtifactKey, Fut>(
        &self,
        executor: &Executor,
        key: Arc<K>,
        body: Fut,
    ) -> Arc<K::Value>
    where
        Fut: Future<Output = K::Value> + Send + 'static,
    {
        let key = UntypedKey::from_arc(key);

        match self.enqueue_async(executor, &key, body) {
            ArtifactState::Resolved(value) => Arc::downcast(value).unwrap(),
            ArtifactState::Pending(waiter) => {
                waiter.wait_async().await.unwrap();
                let guard = self.artifacts.lock().unwrap();
                let Some(ArtifactState::Resolved(value)) = guard.get(&key) else {
                    unreachable!("triggered only when resolved");
                };

                Arc::downcast(value.clone()).unwrap()
            }
        }
    }
}
