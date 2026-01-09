mod expr;
mod fn_map;
mod hmap;
mod key_ref;
mod ordered_index;
mod scope_mutex;
mod storage;

use crate::collections::indexed_map::{
    key_ref::KeyRef,
    ordered_index::OrderedIndexHandle,
    storage::{MapStorage, ReadMapStorageGuard, StorageId},
};

trait ManagedIndex<T> {
    fn insert(&self, storage_guard: &ReadMapStorageGuard<'_, T>, id: StorageId);
    fn remove(&self, storage_guard: &ReadMapStorageGuard<'_, T>, id: StorageId);
}

pub(crate) struct OrderedIndex<K, T> {
    handle: OrderedIndexHandle<K, T>,
}

pub(crate) struct IndexedMap<T> {
    storage: MapStorage<T>,
    indexes: Vec<Box<dyn ManagedIndex<T>>>,
}

impl<T> IndexedMap<T>
where
    T: 'static,
{
    pub(crate) fn new() -> Self {
        Self {
            storage: MapStorage::new(),
            indexes: Vec::new(),
        }
    }

    fn add_index(&mut self, index: impl ManagedIndex<T> + 'static) {
        self.indexes.push(Box::new(index));
    }

    pub(crate) fn add_ordered_index<K>(
        &mut self,
        key_fn: impl Fn(&T) -> KeyRef<'_, K> + 'static,
    ) -> OrderedIndex<K, T>
    where
        K: Ord + 'static,
    {
        let handle = OrderedIndexHandle::new(self.storage.clone(), key_fn);
        self.add_index(handle.clone());
        OrderedIndex { handle }
    }

    pub(crate) fn insert(&mut self, value: T) {
        let id = {
            let mut storage_guard = self.storage.lock_write();
            storage_guard.insert(value)
        };
        let storage_guard = self.storage.lock_read();
        for index in &mut self.indexes {
            index.insert(&storage_guard, id);
        }
    }

    fn remove(&mut self, id: StorageId) {
        {
            let storage_guard = self.storage.lock_read();
            for index in &mut self.indexes {
                index.remove(&storage_guard, id);
            }
        }
        let mut storage_guard = self.storage.lock_write();
        storage_guard.remove_at(id);
    }
}
