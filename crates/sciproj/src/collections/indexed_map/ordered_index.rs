use std::marker::PhantomData;
use std::{borrow::Borrow, fmt::Debug};

use super::{
    expr::IndexPredicate,
    fn_ord_map::FnMultiMap,
    index::ManagedIndex,
    key_ref::{KeyRef, LendingKeyFetcher},
    storage::{MapStorage, StorageId},
};

type KeyFn<K, T> = Box<dyn Fn(&T) -> KeyRef<'_, K>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct IndexOffset(usize);

pub(super) struct OrderedIndexBacking<K, T> {
    index: FnMultiMap<K, StorageId>,
    key_fn: KeyFn<K, T>,
}

impl<K, T> std::fmt::Debug for OrderedIndexBacking<K, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: We should really print more, but we need to do a later pass for that.
        f.debug_struct("OrderedIndexBacking").finish()
    }
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

impl<K, T> LendingKeyFetcher<K, StorageId> for StorageIdKeyFetcher<'_, K, T> {
    fn fetch<'a>(&'a self, value: &'a StorageId) -> KeyRef<'a, K> {
        (self.key_fn)(self.storage.for_id(*value))
    }
}

pub(super) struct Reader<'a, K, T> {
    fetcher: StorageIdKeyFetcher<'a, K, T>,
    index: &'a FnMultiMap<K, StorageId>,
}

impl<K, T> Reader<'_, K, T>
where
    K: Ord,
{
    fn find_eq<Q>(&self, key: &Q) -> impl Iterator<Item = StorageId>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        self.index.get(&self.fetcher, key).copied()
    }

    fn matches<Q>(&self, key: &Q, id: StorageId) -> bool
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        let entry_key = self.fetcher.fetch(&id);
        (*entry_key).borrow() == key
    }
}

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
            fetcher: self.fetcher.clone(),
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

impl<K, T> ManagedIndex<T> for OrderedIndexBacking<K, T>
where
    T: 'static,
    K: Ord + 'static,
{
    fn insert(&mut self, storage: &MapStorage<T>, id: StorageId) {
        self.write_with_storage(storage).insert_id(id);
    }

    fn remove(&mut self, storage: &MapStorage<T>, id: StorageId) {
        self.write_with_storage(storage).remove_id(id);
    }
}

#[derive(Debug)]
pub(super) struct EqPredicate<'a, Q, K> {
    eq_key: &'a Q,
    _phantom: PhantomData<&'a K>,
}

impl<'a, Q, K> EqPredicate<'a, Q, K>
where
    Q: Ord + Debug,
    K: Ord + Borrow<Q> + Debug,
{
    pub(crate) fn new(eq_key: &'a Q) -> Self {
        Self {
            eq_key,
            _phantom: PhantomData,
        }
    }
}

impl<'a, Q, K, T> IndexPredicate<'a, T> for EqPredicate<'a, Q, K>
where
    Q: Ord + Debug,
    K: Ord + Borrow<Q> + Debug + 'static,
    T: Debug + 'static,
{
    type Index = OrderedIndexBacking<K, T>;
    fn find_matching(
        &self,
        index: &Self::Index,
        storage: &MapStorage<T>,
        results: &mut std::collections::HashSet<StorageId>,
    ) {
        results.extend(index.read_with_storage(storage).find_eq(self.eq_key));
    }

    fn matches(&self, index: &Self::Index, storage: &MapStorage<T>, id: StorageId) -> bool {
        index.read_with_storage(storage).matches(self.eq_key, id)
    }
}
