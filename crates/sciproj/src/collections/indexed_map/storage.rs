use std::sync::{Arc, Mutex, MutexGuard};

use slab::Slab;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct StorageId(usize);

struct MapStorageInner<T> {
    entries: Slab<T>,
}

/// The storage for an indexed map. Every value inserted has a unique [`StorageId`].
///
/// Mutability is controlled by [`MapStorage::lock_read`] and [`MapStorage::lock_write`].
pub(super) struct MapStorage<T> {
    inner: MapStorageInner<T>,
}

impl<T> MapStorage<T> {
    pub(super) fn new() -> Self {
        Self {
            inner: MapStorageInner {
                entries: Slab::new(),
            },
        }
    }

    pub(super) fn for_id(&self, index: StorageId) -> &T {
        &self.inner.entries[index.0]
    }

    pub(super) fn insert(&mut self, value: T) -> StorageId {
        StorageId(self.inner.entries.insert(value))
    }

    pub(super) fn remove_at(&mut self, index: StorageId) -> T {
        self.inner.entries.remove(index.0)
    }
}
