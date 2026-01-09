use std::{any::Any, collections::HashMap, marker::PhantomData};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Key<T> {
    id: usize,
    _marker: PhantomData<*const T>,
}

impl<T> Clone for Key<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Key<T> {}

/// A heterogeneous map. Keys are generated when an object is inserted.
pub(super) struct HMap {
    next_id: usize,
    entries: HashMap<usize, Box<dyn Any>>,
}

impl HMap {
    pub(super) fn new() -> Self {
        Self {
            next_id: 0,
            entries: HashMap::new(),
        }
    }

    pub(super) fn insert<T>(&mut self, value: T) -> Key<T>
    where
        T: 'static,
    {
        let id = self.next_id;
        self.next_id = self.next_id.checked_add(1).expect("overflow");
        self.entries.insert(id, Box::new(value));
        Key {
            id,
            _marker: PhantomData,
        }
    }

    pub(super) fn get<T>(&self, key: Key<T>) -> Option<&T>
    where
        T: 'static,
    {
        Some(
            self.entries
                .get(&key.id)?
                .downcast_ref::<T>()
                .expect("key enforces type"),
        )
    }

    pub(super) fn get_mut<T>(&mut self, key: Key<T>) -> Option<&mut T>
    where
        T: 'static,
    {
        Some(
            self.entries
                .get_mut(&key.id)?
                .downcast_mut::<T>()
                .expect("key enforces type"),
        )
    }

    pub(super) fn remove<T>(&mut self, key: Key<T>) -> Option<T>
    where
        T: 'static,
    {
        Some(
            *self
                .entries
                .remove(&key.id)?
                .downcast::<T>()
                .expect("key enforces type"),
        )
    }
}
