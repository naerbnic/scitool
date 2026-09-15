use std::{
    any::Any,
    fmt::Debug,
    hash::{Hash, Hasher},
    sync::Arc,
};

pub(crate) struct UntypedKey(Box<dyn DynKey>);

impl UntypedKey {
    #[must_use]
    pub(crate) fn from_arc<T>(t: Arc<T>) -> Self
    where
        T: Hash + Eq + PartialEq + Send + Sync + 'static,
    {
        Self(Box::new(KeyWrap(t)))
    }
}

impl PartialEq for UntypedKey {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&*other.0)
    }
}

impl Eq for UntypedKey {}

impl Hash for UntypedKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl Debug for UntypedKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("UntypedKey")
            .field(&(&raw const *self.0))
            .finish()
    }
}

impl Clone for UntypedKey {
    fn clone(&self) -> Self {
        Self(self.0.box_clone())
    }
}

trait DynKey: Send + Sync + 'static {
    fn as_any(&self) -> &dyn Any;
    fn eq(&self, other: &dyn DynKey) -> bool;
    fn hash(&self, hasher: &mut dyn Hasher);
    fn box_clone(&self) -> Box<dyn DynKey>;
}

struct KeyWrap<T>(Arc<T>);

impl<T> Clone for KeyWrap<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> DynKey for KeyWrap<T>
where
    T: Hash + Eq + PartialEq + Send + Sync + 'static,
{
    fn as_any(&self) -> &dyn Any {
        let key_ref: &T = &self.0;
        key_ref
    }

    fn eq(&self, other: &dyn DynKey) -> bool {
        let Some(other) = other.as_any().downcast_ref::<T>() else {
            return false;
        };
        &*self.0 == other
    }

    fn hash(&self, mut hasher: &mut dyn Hasher) {
        self.0.hash(&mut hasher);
    }

    fn box_clone(&self) -> Box<dyn DynKey> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_key<T: Hash + Eq + PartialEq + Send + Sync + 'static>(t: T) -> UntypedKey {
        UntypedKey::from_arc(Arc::new(t))
    }

    fn assert_eq_and_hash(k1: &UntypedKey, k2: &UntypedKey) {
        assert_eq!(k1, k2);
        let mut h1 = std::hash::DefaultHasher::new();
        let mut h2 = std::hash::DefaultHasher::new();
        k1.hash(&mut h1);
        k2.hash(&mut h2);
        assert_eq!(h1.finish(), h2.finish());
    }

    #[test]
    fn equal_values_are_equal() {
        let key1 = new_key(42u8);
        let key2 = new_key(42u8);
        assert_eq_and_hash(&key1, &key2);
    }

    #[test]
    fn unequal_values_are_unequal() {
        let key1 = new_key(41u8);
        let key2 = new_key(42u8);
        assert_ne!(key1, key2);
    }

    #[test]
    fn different_types_are_unequal() {
        let key1 = new_key(42u8);
        let key2 = new_key(42u16);
        assert_ne!(key1, key2);
    }
}
