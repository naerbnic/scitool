use std::{
    borrow::Borrow,
    hash::{DefaultHasher, Hash, Hasher as _},
    marker::PhantomData,
};

use hashbrown::{HashSet, HashTable, hash_table::OccupiedEntry};

use super::key_ref::LendingKeyFetcher;

fn get_first<T>(hash_set: &HashSet<T>) -> &T {
    hash_set.iter().next().expect("Empty HashSet")
}

fn hash_value<T>(value: &T) -> u64
where
    T: Hash,
{
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

/// A hash map that hashes by an value _associated_ with the value, as accessed
/// by a function.
///
/// For our design purposes, we allow T to be Copy (as it will likely be some
/// type of reference or pointer), and it still should be Eq + Hash in order to
/// be stored in an equivalence set. Thet type K should be Eq + Hash in order
/// to be used as the primary key in the hash table.
pub(super) struct FnHashMultiMap<K, T> {
    contents: HashTable<HashSet<T>>,
    _phantom: PhantomData<K>,
}

impl<K, T> FnHashMultiMap<K, T>
where
    T: Eq + Hash,
    K: Eq + Hash,
{
    pub(super) fn new() -> Self {
        Self {
            contents: HashTable::new(),
            _phantom: PhantomData,
        }
    }

    pub(super) fn insert<'key>(&mut self, fetcher: &impl LendingKeyFetcher<K, T>, value: T)
    where
        K: Eq + Hash + 'key,
    {
        let new_key_hash = hash_value(&*fetcher.fetch(&value));
        let key_eq_fn = |set: &HashSet<T>| fetcher.fetch(get_first(set)) == fetcher.fetch(&value);
        let entry = self.contents.entry(new_key_hash, key_eq_fn, |set| {
            hash_value(&*fetcher.fetch(get_first(set)))
        });
        entry
            .or_insert_with(|| HashSet::new())
            .get_mut()
            .insert(value);
    }

    pub(super) fn contains<'key>(&self, fetcher: &impl LendingKeyFetcher<K, T>, value: &T) -> bool
    where
        K: 'key,
    {
        let key = fetcher.fetch(value);
        self.key_entry(fetcher, &*key).is_some()
    }

    pub(super) fn contains_key<'key, Q>(
        &self,
        get_key: &impl LendingKeyFetcher<K, T>,
        value: &Q,
    ) -> bool
    where
        K: Borrow<Q> + 'key,
        Q: Eq + Hash,
    {
        self.key_entry(get_key, value).is_some()
    }

    pub(super) fn get<'key, Q>(
        &self,
        fetcher: &impl LendingKeyFetcher<K, T>,
        value: &Q,
    ) -> Option<impl Iterator<Item = &T>>
    where
        K: Borrow<Q> + 'key,
        Q: Eq + Hash,
    {
        Some(self.key_entry(fetcher, value)?.iter())
    }

    pub(super) fn remove(&mut self, fetcher: &impl LendingKeyFetcher<K, T>, value: &T) -> bool {
        let key_hash = hash_value(&*fetcher.fetch(value));
        let key_eq_fn = |set: &HashSet<T>| fetcher.fetch(get_first(set)) == fetcher.fetch(value);
        let Ok(mut hash_set) = self.contents.find_entry(key_hash, key_eq_fn) else {
            return false;
        };

        let hash_set_mut = hash_set.get_mut();

        let removed = hash_set_mut.remove(value);
        if hash_set_mut.is_empty() {
            hash_set.remove();
        }
        removed
    }

    pub(super) fn remove_by_key<Q>(
        &mut self,
        fetcher: &impl LendingKeyFetcher<K, T>,
        value: &Q,
    ) -> Option<impl Iterator<Item = T>>
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        let (hash_set, _) = self.key_entry_mut(fetcher, value)?.remove();
        Some(hash_set.into_iter())
    }

    fn key_entry<'key, Q>(
        &self,
        fetcher: &impl LendingKeyFetcher<K, T>,
        value: &Q,
    ) -> Option<&HashSet<T>>
    where
        K: Borrow<Q> + 'key,
        Q: Eq + Hash,
    {
        let key_hash = hash_value(value);
        let key_eq_fn = |set: &HashSet<T>| (*fetcher.fetch(get_first(set))).borrow() == value;
        self.contents.find(key_hash, key_eq_fn)
    }

    fn key_entry_mut<Q>(
        &mut self,
        fetcher: &impl LendingKeyFetcher<K, T>,
        value: &Q,
    ) -> Option<OccupiedEntry<'_, HashSet<T>>>
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        let key_hash = hash_value(value);
        let key_eq_fn = |set: &HashSet<T>| (*fetcher.fetch(get_first(set))).borrow() == value;
        self.contents.find_entry(key_hash, key_eq_fn).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::super::key_ref::KeyRef;
    use super::*;

    struct BorrowFetcher;

    impl<'s> LendingKeyFetcher<&'s str, &'s str> for BorrowFetcher {
        fn fetch<'a>(&'a self, value: &'a &'s str) -> KeyRef<'a, &'s str> {
            KeyRef::Borrowed(value)
        }
    }

    struct OwnedFetcher(Box<u32>);

    impl LendingKeyFetcher<u32, &str> for OwnedFetcher {
        fn fetch<'a>(&'a self, _value: &'a &str) -> KeyRef<'a, u32> {
            KeyRef::Borrowed(&*self.0)
        }
    }

    #[test]
    fn test_basic_with_func_borrow() {
        let mut map = FnHashMultiMap::new();
        let int_box = Box::new(1u32);
        let fetcher = OwnedFetcher(int_box);
        map.insert(&fetcher, "a");
        map.insert(&fetcher, "b");
        map.insert(&fetcher, "c");
        let result = map.get(&fetcher, &1).unwrap().collect::<HashSet<_>>();
        assert_eq!(result, ["a", "b", "c"].iter().collect::<HashSet<_>>());
    }

    #[test]
    fn test_basic_with_self_borrow() {
        let mut map = FnHashMultiMap::new();
        map.insert(&BorrowFetcher, "a");
        map.insert(&BorrowFetcher, "b");
        map.insert(&BorrowFetcher, "c");
        let result = map
            .get(&BorrowFetcher, &"a")
            .unwrap()
            .collect::<HashSet<_>>();
        assert_eq!(result, ["a"].iter().collect::<HashSet<_>>());
    }

    #[test]
    fn removes_right_entry_with_equal_keys() {}
}
