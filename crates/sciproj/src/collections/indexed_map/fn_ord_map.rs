use std::{cmp::Ordering, marker::PhantomData};

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

fn lower_bound_ord<K, T>(fetcher: &impl LendingKeyFetcher<K, T>, a: &T, b: &T) -> std::cmp::Ordering
where
    K: Ord,
{
    let a_key = fetcher.fetch(a);
    let b_key = fetcher.fetch(b);
    a_key.cmp(&b_key).then(Ordering::Less)
}

fn upper_bound_ord<K, T>(fetcher: &impl LendingKeyFetcher<K, T>, a: &T, b: &T) -> std::cmp::Ordering
where
    K: Ord,
{
    let a_key = fetcher.fetch(a);
    let b_key = fetcher.fetch(b);
    a_key.cmp(&b_key).then(Ordering::Greater)
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

    pub(super) fn get(
        &self,
        fetcher: &impl LendingKeyFetcher<K, T>,
        value: &T,
    ) -> impl Iterator<Item = &T> {
        let Err(lower_bound) = self
            .contents
            .binary_search_by(|v| lower_bound_ord(fetcher, v, value))
        else {
            panic!("Should never be equal")
        };

        let Err(upper_bound) = self
            .contents
            .binary_search_by(|v| upper_bound_ord(fetcher, v, value))
        else {
            panic!("Should never be equal")
        };

        self.contents
            .focus()
            .narrow(lower_bound..upper_bound)
            .into_iter()
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
