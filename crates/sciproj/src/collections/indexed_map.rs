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
mod unique_ordered_index;
mod unique_token;

use std::{
    borrow::Borrow,
    collections::{HashSet, hash_set},
    fmt::Debug,
};

use self::{
    expr::{Predicate, PredicateContext, UniqueEntry},
    index::{IndexInsertError, ManagedIndex},
    index_table::{IndexId, IndexTable, RawIndexId},
    key_ref::KeyRef,
    ordered_index::OrderedIndexBacking,
    storage::{MapStorage, StorageId},
    unique_ordered_index::UniqueOrderedIndexBacking,
};

mod sealed {
    /// module-private type that limits implementabiltiy of a trait to this crate
    pub(crate) struct Sealed {}
}

pub(crate) trait UniqueIndex {
    #[doc(hidden)]
    fn get_index_id(&self) -> &RawIndexId;

    #[doc(hidden)]
    fn sealed(_: sealed::Sealed);
}

#[derive(Debug, thiserror::Error)]
#[error("Failed to insert value into index")]
pub(crate) struct Error {
    error: Box<dyn std::error::Error + Send + Sync>,
}

impl Error {
    pub(crate) fn new(error: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self {
            error: Box::new(error),
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Failed to insert value into index")]
pub(crate) struct InsertError<T> {
    value: T,
}

impl<T> InsertError<T> {
    pub(crate) fn into_value(self) -> T {
        self.value
    }
}

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

pub(crate) struct UniqueOrderedIndex<K, T> {
    id: IndexId<UniqueOrderedIndexBacking<K, T>>,
}

impl<K, T> UniqueOrderedIndex<K, T>
where
    K: Ord,
{
    pub(crate) fn key_expr<'a, Q>(&self, key: &'a Q) -> UniqueEntry<'a, T>
    where
        K: Borrow<Q> + Debug + 'static,
        Q: Ord + Debug,
        T: Debug + 'static,
    {
        UniqueEntry::new(self.id.clone(), unique_ordered_index::EqPredicate::new(key))
    }
}

impl<K, T> UniqueIndex for UniqueOrderedIndex<K, T> {
    fn get_index_id(&self) -> &RawIndexId {
        self.id.get_raw()
    }

    fn sealed(_: sealed::Sealed) {}
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

#[must_use]
pub(crate) struct EntryGuard<'a, T>
where
    T: 'static,
{
    map: &'a mut IndexedMap<T>,
    id: StorageId,
    resolved: bool,
}

impl<T> EntryGuard<'_, T> {
    /// Consumes the guard, and attempts to reindex it. If there is a collision,
    /// it will return an error.
    ///
    /// On an error, the value will
    fn try_resolve_impl(this: &mut Self) -> Result<(), T> {
        assert!(!this.resolved);
        if this.resolved {
            return Ok(());
        }
        this.resolved = true;
        match this.map.reindex_id(this.id) {
            Ok(()) => Ok(()),
            Err(_) => Err(this.map.storage.remove_at(this.id)),
        }
    }

    pub(crate) fn try_resolve(mut this: Self) -> Result<(), T> {
        assert!(!this.resolved);
        Self::try_resolve_impl(&mut this)
    }
}

impl<T> std::ops::Deref for EntryGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.map.storage.for_id(self.id)
    }
}

impl<T> std::ops::DerefMut for EntryGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.map.storage.for_id_mut(self.id)
    }
}

impl<T> Drop for EntryGuard<'_, T>
where
    T: 'static,
{
    fn drop(&mut self) {
        assert!(
            Self::try_resolve_impl(self).is_ok(),
            "Error occured during entry guard drop"
        );
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

    fn add_index<Idx>(&mut self, mut index: Idx) -> Result<IndexId<Idx>, IndexInsertError>
    where
        Idx: ManagedIndex<T>,
    {
        for id in self.storage.all_ids() {
            index.insert(&self.storage, id)?;
        }
        Ok(self.indexes.insert(index))
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
        let id = self
            .add_index(ordered_index::OrderedIndexBacking::new(key_fn))
            .expect("Can't have key collisions for ordered indexes.");
        OrderedIndex { id }
    }

    pub(crate) fn add_unique_ordered_index<K>(
        &mut self,
        key_fn: impl Fn(&T) -> KeyRef<'_, K> + 'static,
    ) -> Result<UniqueOrderedIndex<K, T>, Error>
    where
        K: Ord + 'static,
    {
        let id = self
            .add_index(unique_ordered_index::UniqueOrderedIndexBacking::new(key_fn))
            .map_err(Error::new)?;
        Ok(UniqueOrderedIndex { id })
    }

    /// Inserts a new value into the map. If there are any unique indexes, or
    /// other validation errors, it will report an error.
    pub(crate) fn insert_new(&mut self, value: T) -> Result<(), InsertError<T>> {
        let id = self.storage.insert(value);
        if self.reindex_id(id).is_err() {
            let value = self.storage.remove_at(id);
            return Err(InsertError { value });
        }
        Ok(())
    }

    pub(crate) fn insert_or_update<Idx>(
        &mut self,
        value: T,
        index: &Idx,
    ) -> Result<(), InsertError<T>>
    where
        Idx: UniqueIndex,
    {
        let index_id = index.get_index_id();
        let index = self.indexes.get_raw(index_id).unwrap();

        if let Some(conflict_id) = index.get_conflict(&self.storage, &value) {
            // Since this is an upsert, a collision is equivalent to an overwrite.
            // Remove the old value, and insert the new value.
            self.remove_by_id(std::iter::once(conflict_id));
        }
        self.insert_new(value)
    }

    #[expect(clippy::extra_unused_lifetimes)]
    fn remove_by_id<'a>(&mut self, ids: impl Iterator<Item = StorageId> + Clone) {
        // Remove entries for removed items from all indexes
        for index in self.indexes.values_mut() {
            for id in ids.clone() {
                index.remove(&self.storage, id);
            }
        }
        for id in ids {
            self.storage.remove_at(id);
        }
    }

    /// Removes all values from the map that satisfy the given `IndexExpr`.
    pub(crate) fn remove(&mut self, expr: &Predicate<'_, T>) {
        let mut ids = HashSet::new();
        expr.collect(
            &PredicateContext::new(&self.indexes, &self.storage),
            &mut ids,
        );

        self.remove_by_id(ids.iter().copied());
    }

    fn as_predicate_context(&self) -> PredicateContext<'_, T> {
        PredicateContext::new(&self.indexes, &self.storage)
    }

    fn reindex_id(&mut self, id: StorageId) -> Result<(), IndexInsertError> {
        let mut err_func = || {
            for index in self.indexes.values_mut() {
                index.insert(&self.storage, id)?;
            }
            Ok(())
        };
        let result: Result<(), IndexInsertError> = err_func();
        if result.is_err() {
            // Cleanup indexes, as the item will be removed from storage
            for index in self.indexes.values_mut() {
                index.remove(&self.storage, id);
            }
        }
        result
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
    pub(crate) fn query_mut(&mut self, expr: &Predicate<'_, T>) -> impl LendingIterator<Item = T> {
        let mut ids = HashSet::new();
        expr.collect(&self.as_predicate_context(), &mut ids);

        QueryMut {
            map: self,
            ids: ids.into_iter(),
            prev_id: None,
        }
    }

    pub(crate) fn get(&self, entry: &UniqueEntry<T>) -> Option<&T> {
        let id = entry.get(&self.as_predicate_context())?;
        Some(self.storage.for_id(id))
    }

    pub(crate) fn get_mut(&mut self, entry: &UniqueEntry<T>) -> Option<EntryGuard<'_, T>> {
        let id = entry.get(&self.as_predicate_context())?;

        for index in self.indexes.values_mut() {
            index.remove(&self.storage, id);
        }

        Some(EntryGuard {
            map: self,
            id,
            resolved: false,
        })
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
    fn reindex(&mut self) -> Result<(), Error> {
        if let Some(prev_id) = self.prev_id {
            for index in self.map.indexes.values_mut() {
                index
                    .insert(&self.map.storage, prev_id)
                    .map_err(Error::new)?;
            }
        }
        self.prev_id = None;
        Ok(())
    }
}

impl<T> LendingIterator for QueryMut<'_, T>
where
    T: 'static,
{
    type Item = T;

    fn next(&mut self) -> Option<&mut Self::Item> {
        self.reindex().expect("Failed to reindex");
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
        self.reindex().expect("Failed to reindex");
    }
}

#[cfg(test)]
mod tests {
    use crate::helpers::test::assert_matches;

    use super::*;

    #[test]
    fn test_indexed_map_access() -> Result<(), InsertError<u32>> {
        let mut map = IndexedMap::new();
        map.insert_new(1u32)?;
        map.insert_new(2u32)?;
        map.insert_new(3u32)?;
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
        Ok(())
    }

    #[test]
    fn test_indexed_map_query_mut() -> Result<(), InsertError<u32>> {
        let mut map = IndexedMap::new();
        map.insert_new(1u32)?;
        map.insert_new(2u32)?;
        map.insert_new(3u32)?;
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
        Ok(())
    }

    #[test]
    fn test_unique_index_get() -> anyhow::Result<()> {
        let mut map = IndexedMap::new();
        map.insert_new(1u32)?;
        map.insert_new(2u32)?;
        map.insert_new(3u32)?;
        let index = map.add_unique_ordered_index(KeyRef::from_borrowed_fn(|x| x))?;
        let expr = index.key_expr(&1);
        {
            let mut mut_ref = map.get_mut(&expr).unwrap();
            assert_eq!(*mut_ref, 1);
            *mut_ref = 5;
        }
        assert!(map.get(&index.key_expr(&1)).is_none());
        assert_matches!(map.get(&index.key_expr(&5)), Some(5));
        Ok(())
    }

    #[test]
    fn test_upsert_works() -> anyhow::Result<()> {
        let mut map = IndexedMap::new();
        map.insert_new((1u32, "a"))?;
        map.insert_new((2u32, "b"))?;
        map.insert_new((3u32, "c"))?;
        let index = map.add_unique_ordered_index(KeyRef::from_borrowed_fn(|(x, _)| x))?;
        let expr = index.key_expr(&1);
        {
            let entry_ref = map.get(&expr).unwrap();
            assert_eq!(*entry_ref, (1, "a"));
            map.insert_or_update((1, "d"), &index)?;
        }
        assert_eq!(map.get(&index.key_expr(&1)).unwrap().1, "d");
        Ok(())
    }
}
