use std::cmp::Ordering;

pub(crate) enum KeyRef<'a, K> {
    Owned(K),
    Borrowed(&'a K),
}

impl<K> PartialEq for KeyRef<'_, K>
where
    K: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        PartialEq::eq(&**self, &**other)
    }
}

impl<K> Eq for KeyRef<'_, K> where K: Eq {}

impl<K> PartialOrd for KeyRef<'_, K>
where
    K: PartialOrd,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        PartialOrd::partial_cmp(&**self, &**other)
    }
}

impl<K> Ord for KeyRef<'_, K>
where
    K: Ord,
{
    fn cmp(&self, other: &Self) -> Ordering {
        Ord::cmp(&**self, &**other)
    }
}

impl<K> PartialEq<K> for KeyRef<'_, K>
where
    K: PartialEq,
{
    fn eq(&self, other: &K) -> bool {
        PartialEq::eq(&**self, other)
    }
}

impl<K> PartialOrd<K> for KeyRef<'_, K>
where
    K: PartialOrd,
{
    fn partial_cmp(&self, other: &K) -> Option<Ordering> {
        PartialOrd::partial_cmp(&**self, other)
    }
}

impl<K> std::ops::Deref for KeyRef<'_, K> {
    type Target = K;

    fn deref(&self) -> &Self::Target {
        match self {
            KeyRef::Owned(k) => k,
            KeyRef::Borrowed(k) => k,
        }
    }
}

impl<K> From<K> for KeyRef<'_, K> {
    fn from(k: K) -> Self {
        KeyRef::Owned(k)
    }
}

impl<'a, K> From<&'a K> for KeyRef<'a, K> {
    fn from(k: &'a K) -> Self {
        KeyRef::Borrowed(k)
    }
}

/// A type for fetching a key from a value.
///
/// The result key value can be owned or borrowed, but if borrowed will
/// have a lifetime limited by the fetcher.
pub(super) trait LendingKeyFetcher<K, T> {
    fn fetch<'a, 'val>(&'a self, value: &'val T) -> KeyRef<'a, K>
    where
        'val: 'a;
}
