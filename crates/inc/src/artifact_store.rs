use std::{
    collections::{HashMap, hash_map},
    sync::{Arc, Mutex},
};

use crate::{
    Artifact, ArtifactKey,
    executor::Executor,
    promise::{AnyPromise, Promise},
    untyped_key::UntypedKey,
};

pub(crate) struct ArtifactStore {
    artifacts: Mutex<HashMap<UntypedKey, AnyPromise>>,
}

impl ArtifactStore {
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            artifacts: Mutex::default(),
        }
    }

    pub(crate) fn enqueue_sync<T, F>(
        &self,
        executor: &Executor,
        key: &UntypedKey,
        body: F,
    ) -> Promise<T>
    where
        T: Artifact,
        F: FnOnce() -> T + Send + 'static,
    {
        let mut guard = self.artifacts.lock().unwrap();
        match guard.entry(key.clone()) {
            hash_map::Entry::Occupied(entry) => entry.get().clone().downcast().ok().unwrap(),
            hash_map::Entry::Vacant(vacant_entry) => {
                let (promise, fulfiller) = Promise::new();
                executor.schedule_async({
                    async move {
                        fulfiller.fulfill(Arc::new(body()));
                    }
                });
                vacant_entry.insert(promise.to_any());
                promise
            }
        }
    }

    pub(crate) fn enqueue_async<T, Fut>(
        &self,
        executor: &Executor,
        key: &UntypedKey,
        body: Fut,
    ) -> Promise<T>
    where
        T: Artifact,
        Fut: Future<Output = T> + Send + 'static,
    {
        let mut guard = self.artifacts.lock().unwrap();
        match guard.entry(key.clone()) {
            hash_map::Entry::Occupied(entry) => entry.get().downcast_ref().unwrap().clone(),
            hash_map::Entry::Vacant(vacant_entry) => {
                let (promise, fulfiller) = Promise::new();
                executor.schedule_async({
                    async move {
                        fulfiller.fulfill(Arc::new(body.await));
                    }
                });
                vacant_entry.insert(promise.to_any());
                promise
            }
        }
    }
}
