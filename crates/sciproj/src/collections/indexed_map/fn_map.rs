use im::Vector;

use crate::collections::indexed_map::key_ref::KeyRef;

pub(super) type KeyFn<K, T> = dyn Fn(&T) -> KeyRef<'_, K> + 'static;

/// A map that uses a function to generate keys from the values.
pub(super) struct FnMultiMap<K, T> {
    key_fn: Box<KeyFn<K, T>>,
    contents: Vector<T>,
}

impl<K, T> FnMultiMap<K, T>
where
    T: Ord + Clone,
{
    pub(super) fn new(key_fn: impl Fn(&T) -> KeyRef<'_, K> + 'static) -> Self {
        Self {
            key_fn: Box::new(key_fn),
            contents: Vector::new(),
        }
    }

    pub(super) fn insert(&mut self, value: T) {
        let id = self.contents.len();
        self.contents.push_back(value);
    }
}
