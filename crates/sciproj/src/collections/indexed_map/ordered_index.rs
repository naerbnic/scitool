use std::{
    cmp::Ordering,
    sync::{Arc, Mutex, MutexGuard},
};

use im::Vector;

use crate::collections::indexed_map::{
    ManagedIndex, MapStorage, ReadMapStorageGuard, StorageId, key_ref::KeyRef,
};

type KeyFn<K, T> = Box<dyn Fn(&T) -> KeyRef<'_, K>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct IndexOffset(usize);

pub(super) struct OrderedIndexBacking<K, T> {
    index: Mutex<Vector<StorageId>>,
    key_fn: KeyFn<K, T>,
}

impl<K, T> OrderedIndexBacking<K, T>
where
    K: Ord,
{
    pub(super) fn new(key_fn: impl Fn(&T) -> KeyRef<'_, K> + 'static) -> Self {
        Self {
            index: Mutex::new(Vector::new()),
            key_fn: Box::new(key_fn),
        }
    }

    pub(super) fn lock<'a, 'b>(
        &'a self,
        storage_guard: &'a ReadMapStorageGuard<'b, T>,
    ) -> Guard<'a, 'b, K, T> {
        let index_guard = self.index.lock().unwrap();
        Guard {
            storage_guard,
            index_guard,
            key_fn: &*self.key_fn,
        }
    }
}

struct Guard<'a, 'b, K, T> {
    storage_guard: &'a ReadMapStorageGuard<'b, T>,
    index_guard: MutexGuard<'a, Vector<StorageId>>,
    key_fn: &'a (dyn Fn(&T) -> KeyRef<'_, K> + 'static),
}

impl<K, T> Guard<'_, '_, K, T>
where
    K: Ord,
{
    pub(super) fn id_to_key(&self, slab_index: StorageId) -> KeyRef<'_, K> {
        let entry = self.storage_guard.for_id(slab_index);
        (self.key_fn)(entry)
    }

    fn cmp_ids(&self, a: StorageId, b: StorageId) -> Ordering {
        // First, order by the key.
        let a_key = self.id_to_key(a);
        let b_key = self.id_to_key(b);
        a_key.cmp(&b_key).then_with(|| a.cmp(&b))
    }

    // Insert the storage ID into the index, in the correct position.
    pub(super) fn insert_id(&mut self, slab_index: StorageId) {
        let insert_index = {
            self.index_guard
                .binary_search_by(|id: &StorageId| self.cmp_ids(*id, slab_index))
                .expect_err("id is already in the index")
        };
        self.index_guard.insert(insert_index, slab_index);
    }

    pub(super) fn remove_id(&mut self, slab_index: StorageId) {
        let remove_index = {
            self.binary_search_by(|id| self.cmp_ids(id, slab_index))
                .expect("id is not in the index")
        };
        self.index_guard.remove(remove_index.0);
    }

    fn binary_search_by<F>(&self, mut f: F) -> Result<IndexOffset, IndexOffset>
    where
        F: FnMut(StorageId) -> Ordering,
    {
        self.index_guard
            .binary_search_by(|id: &StorageId| f(*id))
            .map(IndexOffset)
            .map_err(IndexOffset)
    }

    pub(super) fn lower_bound(&self, key: &K) -> IndexOffset {
        self.binary_search_by(|id| {
            let entry_key = self.id_to_key(id);
            (&*entry_key).cmp(key).then(Ordering::Less)
        })
        .expect_err("lower_bound search never is eq")
    }

    pub(super) fn upper_bound(&self, key: &K) -> IndexOffset {
        self.binary_search_by(|id| {
            let entry_key = self.id_to_key(id);
            (&*entry_key).cmp(key).then(Ordering::Greater)
        })
        .expect_err("upper_bound search never is eq")
    }
}

pub(super) struct OrderedIndexHandle<K, T> {
    storage: MapStorage<T>,
    backing: Arc<OrderedIndexBacking<K, T>>,
}

impl<K, T> OrderedIndexHandle<K, T>
where
    K: Ord,
{
    pub(super) fn new(
        storage: MapStorage<T>,
        key_fn: impl Fn(&T) -> KeyRef<'_, K> + 'static,
    ) -> Self {
        Self {
            storage,
            backing: Arc::new(OrderedIndexBacking::new(key_fn)),
        }
    }
}

impl<K, T> ManagedIndex<T> for OrderedIndexHandle<K, T>
where
    K: Ord,
{
    fn insert(&self, storage_guard: &ReadMapStorageGuard<'_, T>, id: StorageId) {
        self.backing.lock(storage_guard).insert_id(id);
    }

    fn remove(&self, storage_guard: &ReadMapStorageGuard<'_, T>, id: StorageId) {
        self.backing.lock(storage_guard).remove_id(id);
    }
}

impl<K, T> Clone for OrderedIndexHandle<K, T> {
    fn clone(&self) -> Self {
        Self {
            storage: self.storage.clone(),
            backing: self.backing.clone(),
        }
    }
}
