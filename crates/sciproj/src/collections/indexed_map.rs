#![cfg_attr(not(test), expect(dead_code, reason = "In development"))]
#![cfg_attr(test, expect(dead_code, reason = "To be tested"))]

mod expr;
mod fn_hash_map;
mod fn_ord_map;
mod hmap;
mod index;
mod index_table;
mod key_ref;
mod ordered_index;
mod storage;
mod unique_token;

use std::{
    borrow::Borrow,
    collections::{HashSet, hash_set},
    fmt::Debug,
};

use self::{
    expr::{Predicate, PredicateContext},
    index::ManagedIndex as _,
    index_table::{IndexId, IndexTable},
    key_ref::KeyRef,
    ordered_index::OrderedIndexBacking,
    storage::{MapStorage, StorageId},
};

/// A handle to an ordered index that is associated with a specific `IndexedMap`.
///
/// This can be used to create a query against this index, which can be used in
/// operations on `IndexedMap`.
///
/// Values of this type, and any values that are created from it, are associated
/// with the `IndexedMap` that created them. If an `OrderedIndex` is used with a
/// different instance of `IndexedMap`, it will cause a panic, or other
/// unspecified (but not unsafe) behavior.
pub(crate) struct OrderedIndex<K, T> {
    /// Contents to be determined.
    id: IndexId<OrderedIndexBacking<K, T>>,
}

impl<K, T> OrderedIndex<K, T>
where
    K: Ord,
{
    /// Creates a new `IndexExpr` that will select all elements where the key
    /// is equal to the given key.
    pub(crate) fn eq_expr<'a, Q>(&self, key: &'a Q) -> Predicate<'a, T>
    where
        K: Borrow<Q> + Debug + 'static,
        Q: Ord + Debug,
        T: Debug + 'static,
    {
        Predicate::index(self.id.clone(), ordered_index::EqPredicate::new(key))
    }
}

pub(crate) trait LendingIterator {
    type Item;

    fn next(&mut self) -> Option<&mut Self::Item>;
}

impl<T> LendingIterator for &mut [T] {
    type Item = T;

    fn next(&mut self) -> Option<&mut Self::Item> {
        let (head, tail) = std::mem::take(self).split_first_mut()?;
        *self = tail;
        Some(head)
    }
}

/// A data structure that stores values and allows them to be indexed by
/// multiple keys that are derived from the values.
///
/// Adding an index to an `IndexedMap` will cause the index to be populated
/// with the existing values in the map.
pub(crate) struct IndexedMap<T> {
    storage: MapStorage<T>,
    indexes: IndexTable<T>,
}

impl<T> IndexedMap<T>
where
    T: 'static,
{
    /// Creates a new empty `IndexedMap`.
    pub(crate) fn new() -> Self {
        Self {
            storage: MapStorage::new(),
            indexes: IndexTable::new(),
        }
    }

    /// Adds a new index to the map that indexes based on a key type K, which
    /// implements `Ord`. The `key_fn` is used to extract the key from the
    /// value.
    ///
    /// The returned value is associated with this instance of the map. If the
    /// `OrderedIndex` value is used with a different instance of `IndexedMap`,
    /// it will cause a panic.
    pub(crate) fn add_ordered_index<K>(
        &mut self,
        key_fn: impl Fn(&T) -> KeyRef<'_, K> + 'static,
    ) -> OrderedIndex<K, T>
    where
        K: Ord + 'static,
    {
        let mut index = ordered_index::OrderedIndexBacking::new(key_fn);
        for id in self.storage.all_ids() {
            index.insert(&self.storage, id);
        }
        let id = self.indexes.insert(index);
        OrderedIndex { id }
    }

    /// Inserts a value into the map. The value will be added to all indexes
    /// that have been created in the map.
    pub(crate) fn insert(&mut self, value: T) {
        let id = self.storage.insert(value);
        for index in self.indexes.values_mut() {
            index.insert(&self.storage, id);
        }
    }

    /// Removes all values from the map that satisfy the given `IndexExpr`.
    fn remove(&mut self, expr: &Predicate<'_, T>) {
        let mut ids = HashSet::new();
        expr.collect(
            &PredicateContext::new(&self.indexes, &self.storage),
            &mut ids,
        );

        // Remove entries for removed items from all indexes
        for index in self.indexes.values_mut() {
            for id in ids.iter().copied() {
                index.remove(&self.storage, id);
            }
        }

        // Remove items from storage
        #[expect(
            clippy::iter_over_hash_type,
            reason = "This is for a simple slab allocator"
        )]
        for id in ids {
            self.storage.remove_at(id);
        }
    }

    fn as_predicate_context(&self) -> PredicateContext<'_, T> {
        PredicateContext::new(&self.indexes, &self.storage)
    }

    /// Returns an iterator over the values in the map that satisfy the given
    /// `IndexExpr`.
    pub(crate) fn query(&self, expr: &Predicate<'_, T>) -> impl Iterator<Item = &T> {
        let mut ids = HashSet::new();
        expr.collect(&self.as_predicate_context(), &mut ids);

        ids.into_iter().map(|id| self.storage.for_id(id))
    }

    /// Returns an iterator over the mutable values in the map that satisfy the
    /// given `IndexExpr`.
    ///
    /// Note that, because mutating the value may invalidate its entry in its
    /// indexes, any values that are yielded here will be re-indexed after the
    /// iterator is dropped. If the iterator is forgotten, that may result in
    /// the indexes being out of sync with the storage, causing unspecified
    /// (but safe) behavior.
    fn query_mut(&mut self, expr: &Predicate<'_, T>) -> impl LendingIterator<Item = T> {
        let mut ids = HashSet::new();
        expr.collect(&self.as_predicate_context(), &mut ids);

        QueryMut {
            map: self,
            ids: ids.into_iter(),
            prev_id: None,
        }
    }
}

pub(crate) struct QueryMut<'a, T>
where
    T: 'static,
{
    map: &'a mut IndexedMap<T>,
    ids: hash_set::IntoIter<StorageId>,
    prev_id: Option<StorageId>,
}

impl<T> QueryMut<'_, T>
where
    T: 'static,
{
    /// If there is a pending re-indexing operation, perform it.
    ///
    /// Idempotent.
    fn reindex(&mut self) {
        if let Some(prev_id) = self.prev_id {
            for index in self.map.indexes.values_mut() {
                index.insert(&self.map.storage, prev_id);
            }
        }
        self.prev_id = None;
    }
}

impl<T> LendingIterator for QueryMut<'_, T>
where
    T: 'static,
{
    type Item = T;

    fn next(&mut self) -> Option<&mut Self::Item> {
        self.reindex();
        let next_id = self.ids.next()?;
        // Remove the id from all indexes, in preparation for re-indexing
        for index in self.map.indexes.values_mut() {
            index.remove(&self.map.storage, next_id);
        }
        self.prev_id = Some(next_id);
        Some(self.map.storage.for_id_mut(next_id))
    }
}

impl<T> Drop for QueryMut<'_, T>
where
    T: 'static,
{
    fn drop(&mut self) {
        self.reindex();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_indexed_map_access() {
        let mut map = IndexedMap::new();
        map.insert(1u32);
        map.insert(2u32);
        map.insert(3u32);
        let index = map.add_ordered_index(KeyRef::from_borrowed_fn(|x| x));
        assert_eq!(
            map.query(&index.eq_expr(&1)).copied().collect::<Vec<_>>(),
            vec![1]
        );
        assert_eq!(
            map.query(&index.eq_expr(&2)).copied().collect::<Vec<_>>(),
            vec![2]
        );
        assert_eq!(
            map.query(&index.eq_expr(&3)).copied().collect::<Vec<_>>(),
            vec![3]
        );
    }

    #[test]
    fn test_indexed_map_query_mut() {
        let mut map = IndexedMap::new();
        map.insert(1u32);
        map.insert(2u32);
        map.insert(3u32);
        let index = map.add_ordered_index(KeyRef::from_borrowed_fn(|x| x));
        let expr = index.eq_expr(&1);
        {
            let mut query = map.query_mut(&expr);
            let mut_ref = query.next().unwrap();
            assert_eq!(*mut_ref, 1);
            *mut_ref = 5;
        }
        assert_eq!(map.query(&index.eq_expr(&1)).count(), 0);
        assert_eq!(map.query(&index.eq_expr(&5)).count(), 1);
    }
}
