use std::{
    any::{Any, TypeId},
    collections::{HashMap, hash_map::Entry},
};

#[derive(Debug, thiserror::Error)]
#[error("Key already exists")]
pub(crate) struct AlreadyExists<T>(T);

impl<T> AlreadyExists<T> {}

pub(crate) struct TypeMap<V> {
    store: HashMap<TypeId, V>,
}

impl<V> TypeMap<V> {
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            store: HashMap::new(),
        }
    }

    pub(crate) fn get<T: Any>(&self) -> Option<&V> {
        self.store.get(&TypeId::of::<T>())
    }

    pub(crate) fn unique_insert<T: Any>(&mut self, value: V) -> Result<(), AlreadyExists<V>> {
        match self.store.entry(TypeId::of::<T>()) {
            Entry::Occupied(_) => Err(AlreadyExists(value)),
            Entry::Vacant(vacant_entry) => {
                vacant_entry.insert(value);
                Ok(())
            }
        }
    }
}

impl<V> Default for TypeMap<V> {
    fn default() -> Self {
        Self::new()
    }
}
