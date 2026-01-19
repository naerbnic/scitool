use std::{borrow::Borrow, cmp::Ordering, marker::PhantomData};

use im::Vector;

use super::key_ref::LendingKeyFetcher;

fn total_ord<K, T>(fetcher: &impl LendingKeyFetcher<K, T>, a: &T, b: &T) -> std::cmp::Ordering
where
    T: Ord,
    K: Ord,
{
    let a_key = fetcher.fetch(a);
    let b_key = fetcher.fetch(b);
    a_key.cmp(&b_key).then_with(|| a.cmp(b))
}

fn lower_bound_searcher<Q, K, T>(
    fetcher: &impl LendingKeyFetcher<K, T>,
    value: &Q,
    entry: &T,
) -> std::cmp::Ordering
where
    K: Borrow<Q>,
    Q: Ord,
{
    let entry_key = fetcher.fetch(entry);
    let entry_key: &Q = (*entry_key).borrow();
    entry_key.cmp(value).then(Ordering::Greater)
}

fn upper_bound_searcher<Q, K, T>(
    fetcher: &impl LendingKeyFetcher<K, T>,
    value: &Q,
    entry: &T,
) -> std::cmp::Ordering
where
    K: Borrow<Q>,
    Q: Ord,
{
    let entry_key = fetcher.fetch(entry);
    let entry_key: &Q = (*entry_key).borrow();
    entry_key.cmp(value).then(Ordering::Less)
}

/// A map that uses an external function to generate keys from the values.
///
/// This does not store the function directly, as the function may itself
/// borrow data externally. For correctness, the function that is provided
/// should have the same semantics each time it is passed to methods on the
/// map.
pub(super) struct FnMultiMap<K, T> {
    contents: Vector<T>,
    _phantom: PhantomData<K>,
}

impl<K, T> FnMultiMap<K, T>
where
    T: Ord + Clone,
    K: Ord,
{
    pub(super) fn new() -> Self {
        Self {
            contents: Vector::new(),
            _phantom: PhantomData,
        }
    }

    pub(super) fn insert(&mut self, fetcher: &impl LendingKeyFetcher<K, T>, value: T) -> &T {
        let index = match self
            .contents
            .binary_search_by(|v| total_ord(fetcher, v, &value))
        {
            Ok(index) => index,
            Err(index) => {
                self.contents.insert(index, value);
                index
            }
        };
        &self.contents[index]
    }

    pub(super) fn get<Q>(
        &self,
        fetcher: &impl LendingKeyFetcher<K, T>,
        value: &Q,
    ) -> impl Iterator<Item = &T>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        let Err(lower_bound) = self
            .contents
            .binary_search_by(|v| lower_bound_searcher(fetcher, value, v))
        else {
            panic!("Should never be equal")
        };

        let Err(upper_bound) = self
            .contents
            .binary_search_by(|v| upper_bound_searcher(fetcher, value, v))
        else {
            panic!("Should never be equal")
        };

        let focus = self.contents.focus();

        if lower_bound == upper_bound {
            // The `im` crate makes it weirdly hard to get an empty focus to
            // iterate over, as it panics if the location is equal to length,
            // even though the standard implementation of `split_at` would
            // allow it, and pass an empty slice as as the second value.
            if focus.is_empty() {
                return focus.into_iter();
            }

            let (empty, _) = focus.split_at(0);
            return empty.into_iter();
        }

        focus.narrow(lower_bound..upper_bound).into_iter()
    }

    pub(super) fn contains(&self, fetcher: &impl LendingKeyFetcher<K, T>, value: &T) -> bool {
        self.contents
            .binary_search_by(|v| total_ord(fetcher, v, value))
            .is_ok()
    }

    pub(super) fn remove(
        &mut self,
        fetcher: &impl LendingKeyFetcher<K, T>,
        value: &T,
    ) -> Option<T> {
        if let Ok(index) = self
            .contents
            .binary_search_by(|v| total_ord(fetcher, v, value))
        {
            Some(self.contents.remove(index))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::key_ref::KeyRef;
    use super::*;

    struct IdentityLendingFn;

    impl<T> LendingKeyFetcher<T, T> for IdentityLendingFn {
        fn fetch<'a>(&'a self, value: &'a T) -> KeyRef<'a, T> {
            KeyRef::Borrowed(value)
        }
    }

    struct ConstLendingFn<K>(K);

    impl<K, T> LendingKeyFetcher<K, T> for ConstLendingFn<K> {
        fn fetch<'a>(&'a self, _value: &'a T) -> KeyRef<'a, K> {
            KeyRef::Borrowed(&self.0)
        }
    }

    #[test]
    fn test_fn_multi_map_insert_singleton() {
        let mut map = FnMultiMap::new();
        map.insert(&IdentityLendingFn, 1);
        let items = map.get(&IdentityLendingFn, &1).copied().collect::<Vec<_>>();
        assert_eq!(items, vec![1]);
    }

    #[test]
    fn test_fn_multi_map() {
        let mut map = FnMultiMap::new();
        map.insert(&IdentityLendingFn, 1);
        map.insert(&IdentityLendingFn, 2);
        map.insert(&IdentityLendingFn, 3);
        assert_eq!(map.get(&IdentityLendingFn, &1).count(), 1);
        assert_eq!(map.get(&IdentityLendingFn, &2).count(), 1);
        assert_eq!(map.get(&IdentityLendingFn, &3).count(), 1);
        assert_eq!(map.get(&IdentityLendingFn, &4).count(), 0);
    }

    #[test]
    fn removes_explicit_entry_with_equal_keys() {
        // Check that we remove the correct entry, even when the key for
        // multiple entries is the same.
        let mut map = FnMultiMap::new();
        let const_fn = ConstLendingFn(1);
        map.insert(&const_fn, 1);
        map.insert(&const_fn, 2);
        map.insert(&const_fn, 3);
        assert_eq!(map.remove(&const_fn, &2), Some(2));
        assert_eq!(
            map.get(&const_fn, &1).copied().collect::<Vec<_>>(),
            vec![1, 3]
        );
    }
}
