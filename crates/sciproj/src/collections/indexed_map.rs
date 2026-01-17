mod expr;
mod fn_hash_map;
mod fn_ord_map;
mod hmap;
mod index;
mod key_ref;
mod ordered_index;
mod scope_mutex;
mod storage;

use std::{borrow::Borrow, marker::PhantomData};

use crate::collections::indexed_map::{
    key_ref::KeyRef,
    storage::{MapStorage, StorageId},
};

/// An index expression, used to select elements from an `IndexedMap`.
pub(crate) struct IndexExpr<'a, T> {
    /// Contents to be determined.
    handle: PhantomData<&'a T>,
}

impl<T> IndexExpr<'_, T> {
    /// Creates a new `IndexExpr` that represents the logical AND of two
    /// `IndexExpr` values. Both expressions must be associated with the same
    /// `IndexedMap`.
    pub(crate) fn and(self, _other: Self) -> Self {
        Self {
            handle: PhantomData,
        }
    }

    /// Creates a new `IndexExpr` that represents the logical OR of two
    /// `IndexExpr` values. Both expressions must be associated with the same
    /// `IndexedMap`.
    pub(crate) fn or(self, _other: Self) -> Self {
        Self {
            handle: PhantomData,
        }
    }

    /// Creates a new `IndexExpr` that represents the logical NOT of an
    /// `IndexExpr` value.
    pub(crate) fn not(self) -> Self {
        Self {
            handle: PhantomData,
        }
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
    handle: PhantomData<(K, T)>,
}

impl<K, T> OrderedIndex<K, T>
where
    K: Ord,
{
    /// Creates a new `IndexExpr` that will select all elements where the key
    /// is equal to the given key.
    pub(crate) fn eq_expr<'a, Q>(&self, _key: &'a Q) -> IndexExpr<'a, T>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        todo!()
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
    indexes: Vec<Box<dyn index::ManagedIndex<T>>>,
}

impl<T> IndexedMap<T>
where
    T: 'static,
{
    /// Creates a new empty `IndexedMap`.
    pub(crate) fn new() -> Self {
        Self {
            storage: MapStorage::new(),
            indexes: Vec::new(),
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
        todo!()
    }

    /// Inserts a value into the map. The value will be added to all indexes
    /// that have been created in the map.
    pub(crate) fn insert(&mut self, value: T) {
        let id = self.storage.insert(value);
        for index in &mut self.indexes {
            index.insert(&self.storage, id);
        }
    }

    /// Removes all values from the map that satisfy the given `IndexExpr`.
    fn remove(&mut self, expr: &IndexExpr<'_, T>) {
        todo!()
    }

    /// Returns an iterator over the values in the map that satisfy the given
    /// `IndexExpr`.
    fn query(&self, expr: &IndexExpr<'_, T>) -> impl Iterator<Item = &T> {
        todo!();
        [].iter()
    }

    /// Returns an iterator over the mutable values in the map that satisfy the
    /// given `IndexExpr`.
    ///
    /// Note that, because mutating the value may invalidate its entry in its
    /// indexes, any values that are yielded here will be re-indexed after the
    /// iterator is dropped. If the iterator is forgotten, that may result in
    /// the indexes being out of sync with the storage, causing unspecified
    /// (but safe) behavior.
    fn query_mut(&mut self, _expr: &IndexExpr<'_, T>) -> impl LendingIterator<Item = T> {
        todo!();
        &mut ([])[..]
    }
}
