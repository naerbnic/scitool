#![allow(unsafe_code)]
use std::{
    borrow::Borrow,
    collections::{HashMap, hash_map},
    hash::Hash,
    pin::Pin,
    ptr,
    sync::Mutex,
};

unsafe fn transmute_lifetime<'b, T>(v: &T) -> &'b T {
    unsafe { &*ptr::from_ref::<T>(v) }
}

/// A shared cache that can provide references to inserted values.
///
/// It's guaranteed that values will not be removed from the cache until
/// the object itself is destroyed, or a mut reference is kept to the object.
#[derive(Debug)]
pub(crate) struct MutCache<K, V> {
    map: Mutex<HashMap<K, Box<V>>>,
}

impl<K, V> MutCache<K, V>
where
    K: Eq + Hash,
    V: Sync,
{
    pub(crate) fn new() -> Self {
        Self {
            map: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn sync_insert(&self, key: K, value: V) -> Result<&V, V> {
        let mut guard = self.map.lock().unwrap();

        match guard.entry(key) {
            hash_map::Entry::Occupied(_) => Err(value),
            hash_map::Entry::Vacant(vac) => {
                let v_ref: &V = vac.insert(Box::new(value));
                // We take a reference on the value that has a lifetime of the
                // outer object, rather than the guard itself.
                //
                // SAFETY:
                // - The value will remain during the entire extent of the
                //   current shared lifetime. Enforced by preconditions.
                // - The reference itself will not change, enforced by reference
                //   stability within a box.
                Ok(unsafe { transmute_lifetime(v_ref) })
            }
        }
    }

    pub(crate) fn get_or_insert_with<F>(&self, key: K, init: F) -> &V
    where
        F: FnOnce() -> V,
    {
        let mut guard = self.map.lock().unwrap();

        let v_ref: &V = guard.entry(key).or_insert_with(|| Box::new(init()));

        // Convert to a reference that has a lifetime of the
        // outer object, rather than the guard itself.
        //
        // SAFETY:
        // - The value will remain during the entire extent of the
        //   current shared lifetime. Enforced by preconditions.
        // - The reference itself will not change, enforced by reference
        //   stability within a box.
        unsafe { transmute_lifetime(v_ref) }
    }

    pub(crate) fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        Q: Hash + Eq + ?Sized,
        K: Borrow<Q>,
    {
        let guard = self.map.lock().unwrap();
        let v_ref: Option<&V> = guard.get(key).map(|v| &**v);
        // SAFETY:
        // - The value will remain live during the entire extent of the
        //   current shared lifetime (if not longer).
        // - The reference itself will not change, enforced by reference
        //   stability within a box.
        v_ref.map(|v| unsafe { transmute_lifetime(v) })
    }

    pub(crate) fn insert(&mut self, key: K, value: V) -> Option<V> {
        let mut guard = self.map.lock().unwrap();
        match guard.entry(key) {
            hash_map::Entry::Occupied(occ) => Some(std::mem::replace(&mut **occ.into_mut(), value)),
            hash_map::Entry::Vacant(vac) => {
                vac.insert(Box::new(value));
                None
            }
        }
    }

    pub(crate) fn remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        Q: Hash + Eq + ?Sized,
        K: Borrow<Q>,
    {
        let mut guard = self.map.lock().unwrap();
        let removed = guard.remove(key)?;
        Some(*removed)
    }
}
