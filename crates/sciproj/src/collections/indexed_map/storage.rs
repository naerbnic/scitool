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
    inner: Arc<Mutex<MapStorageInner<T>>>,
}

impl<T> Clone for MapStorage<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> MapStorage<T> {
    pub(super) fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(MapStorageInner {
                entries: Slab::new(),
            })),
        }
    }

    pub(super) fn lock_read(&self) -> ReadMapStorageGuard<'_, T> {
        ReadMapStorageGuard {
            inner: self.inner.lock().unwrap(),
        }
    }

    pub(super) fn lock_write(&self) -> WriteMapStorageGuard<'_, T> {
        WriteMapStorageGuard {
            inner: self.inner.lock().unwrap(),
        }
    }
}

pub(super) struct ReadMapStorageGuard<'a, T> {
    inner: MutexGuard<'a, MapStorageInner<T>>,
}

impl<T> ReadMapStorageGuard<'_, T> {
    pub(super) fn for_id(&self, index: StorageId) -> &T {
        &self.inner.entries[index.0]
    }
}

pub(super) struct WriteMapStorageGuard<'a, T> {
    inner: MutexGuard<'a, MapStorageInner<T>>,
}

impl<T> WriteMapStorageGuard<'_, T> {
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
