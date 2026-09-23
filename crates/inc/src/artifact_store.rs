use std::sync::Arc;

use crate::{
    Artifact,
    executor::Executor,
    mut_cache::MutCache,
    promise::{SharedAnyPromise, SharedPromise},
    untyped_key::UntypedKey,
};

pub(crate) struct ArtifactStore {
    artifacts: MutCache<UntypedKey, SharedAnyPromise>,
}

impl ArtifactStore {
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            artifacts: MutCache::new(),
        }
    }

    pub(crate) fn insert<T>(&self, key: &UntypedKey, value: T) -> &SharedPromise<T>
    where
        T: Artifact,
    {
        let Ok(promise_ref) = self.artifacts.sync_insert(
            key.clone(),
            SharedPromise::new_ready(Arc::new(value)).into_any(),
        ) else {
            panic!("Artifact already exists");
        };
        promise_ref.downcast_ref::<T>().unwrap()
    }

    pub(crate) fn try_get<T>(&self, key: &UntypedKey) -> Option<&SharedPromise<T>>
    where
        T: Artifact,
    {
        Some(self.artifacts.get(key)?.downcast_ref::<T>().unwrap())
    }

    pub(crate) fn enqueue_sync<T, F>(
        &self,
        executor: &Executor,
        key: &UntypedKey,
        body: F,
    ) -> &SharedPromise<T>
    where
        T: Artifact,
        F: FnOnce() -> T + Send + 'static,
    {
        let any_promise = self.artifacts.get_or_insert_with(key.clone(), || {
            let (promise, fulfiller) = SharedPromise::new();
            executor.schedule_async({
                async move {
                    fulfiller.fulfill_arc(Arc::new(body()));
                }
            });
            promise.into_any()
        });
        any_promise.downcast_ref::<T>().unwrap()
    }

    pub(crate) fn enqueue_async<T, Fut>(
        &self,
        executor: &Executor,
        key: &UntypedKey,
        body: Fut,
    ) -> &SharedPromise<T>
    where
        T: Artifact,
        Fut: Future<Output = T> + Send + 'static,
    {
        let any_promise = self.artifacts.get_or_insert_with(key.clone(), || {
            let (promise, fulfiller) = SharedPromise::new();
            executor.schedule_async({
                async move {
                    fulfiller.fulfill_arc(Arc::new(body.await));
                }
            });
            promise.into_any()
        });
        any_promise.downcast_ref::<T>().unwrap()
    }
}
