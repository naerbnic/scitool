use std::collections::HashSet;

use slab::Slab;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct StorageId(usize);

struct MapStorageInner<T> {
    entries: Slab<T>,
    keys: HashSet<StorageId>,
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
                keys: HashSet::new(),
            },
        }
    }

    pub(super) fn size(&self) -> usize {
        self.inner.keys.len()
    }

    pub(super) fn all_ids(&self) -> impl Iterator<Item = StorageId> {
        self.inner.keys.iter().copied()
    }

    pub(super) fn for_id(&self, index: StorageId) -> &T {
        &self.inner.entries[index.0]
    }

    pub(super) fn for_id_mut(&mut self, index: StorageId) -> &mut T {
        &mut self.inner.entries[index.0]
    }

    pub(super) fn insert(&mut self, value: T) -> StorageId {
        let id = StorageId(self.inner.entries.insert(value));
        self.inner.keys.insert(id);
        id
    }

    pub(super) fn remove_at(&mut self, index: StorageId) -> T {
        self.inner.keys.remove(&index);
        self.inner.entries.remove(index.0)
    }
}
