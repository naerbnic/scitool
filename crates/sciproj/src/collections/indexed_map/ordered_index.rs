use std::{cell::RefCell, rc::Rc};

use crate::collections::indexed_map::{fn_ord_map::FnMultiMap, key_ref::LendingKeyFetcher};

use super::{
    index::ManagedIndex,
    key_ref::KeyRef,
    storage::{MapStorage, StorageId},
};

type KeyFn<K, T> = Box<dyn Fn(&T) -> KeyRef<'_, K>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct IndexOffset(usize);

pub(super) struct OrderedIndexBacking<K, T> {
    index: FnMultiMap<K, StorageId>,
    key_fn: KeyFn<K, T>,
}

impl<K, T> OrderedIndexBacking<K, T>
where
    K: Ord,
{
    pub(super) fn new(key_fn: impl Fn(&T) -> KeyRef<'_, K> + 'static) -> Self {
        Self {
            index: FnMultiMap::new(),
            key_fn: Box::new(key_fn),
        }
    }

    pub(super) fn read_with_storage<'a>(&'a self, storage: &'a MapStorage<T>) -> Reader<'a, K, T> {
        Reader {
            fetcher: StorageIdKeyFetcher {
                storage,
                key_fn: &*self.key_fn,
            },
            index: &self.index,
        }
    }

    pub(super) fn write_with_storage<'a>(
        &'a mut self,
        storage: &'a MapStorage<T>,
    ) -> Writer<'a, K, T> {
        Writer {
            fetcher: StorageIdKeyFetcher {
                storage,
                key_fn: &*self.key_fn,
            },
            index: &mut self.index,
        }
    }
}

struct StorageIdKeyFetcher<'a, K, T> {
    storage: &'a MapStorage<T>,
    key_fn: &'a (dyn Fn(&T) -> KeyRef<'_, K> + 'static),
}

impl<K, T> Clone for StorageIdKeyFetcher<'_, K, T> {
    fn clone(&self) -> Self {
        Self {
            storage: self.storage,
            key_fn: self.key_fn,
        }
    }
}

impl<K, T> Copy for StorageIdKeyFetcher<'_, K, T> {}

impl<K, T> LendingKeyFetcher<K, StorageId> for StorageIdKeyFetcher<'_, K, T> {
    fn fetch<'a, 'val>(&'a self, value: &'val StorageId) -> KeyRef<'a, K>
    where
        'val: 'a,
    {
        let entry = self.storage.for_id(*value);
        (self.key_fn)(&entry)
    }
}

struct Reader<'a, K, T> {
    fetcher: StorageIdKeyFetcher<'a, K, T>,
    index: &'a FnMultiMap<K, StorageId>,
}

impl<K, T> Reader<'_, K, T> where K: Ord {}

pub(super) struct Writer<'a, K, T> {
    fetcher: StorageIdKeyFetcher<'a, K, T>,
    index: &'a mut FnMultiMap<K, StorageId>,
}

impl<K, T> Writer<'_, K, T>
where
    K: Ord,
{
    pub(super) fn as_reader(&self) -> Reader<'_, K, T> {
        Reader {
            fetcher: self.fetcher,
            index: self.index,
        }
    }

    pub(super) fn insert_id(&mut self, slab_index: StorageId) {
        self.index.insert(&self.fetcher, slab_index);
    }

    pub(super) fn remove_id(&mut self, slab_index: StorageId) {
        self.index.remove(&self.fetcher, &slab_index);
    }
}

pub(super) struct OrderedIndexHandle<K, T> {
    backing: Rc<RefCell<OrderedIndexBacking<K, T>>>,
}

impl<K, T> OrderedIndexHandle<K, T>
where
    K: Ord,
{
    pub(super) fn new(key_fn: impl Fn(&T) -> KeyRef<'_, K> + 'static) -> Self {
        Self {
            backing: Rc::new(RefCell::new(OrderedIndexBacking::new(key_fn))),
        }
    }
}

impl<K, T> ManagedIndex<T> for OrderedIndexHandle<K, T>
where
    K: Ord,
{
    fn insert(&mut self, storage: &MapStorage<T>, id: StorageId) {
        self.backing
            .borrow_mut()
            .write_with_storage(storage)
            .insert_id(id);
    }

    fn remove(&mut self, storage: &MapStorage<T>, id: StorageId) {
        self.backing
            .borrow_mut()
            .write_with_storage(storage)
            .remove_id(id);
    }
}

impl<K, T> Clone for OrderedIndexHandle<K, T> {
    fn clone(&self) -> Self {
        Self {
            backing: self.backing.clone(),
        }
    }
}
