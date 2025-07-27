use std::{
    ops::Deref,
    sync::{Arc, Weak},
};

trait CacheCostEvaluator<T> {
    fn eval_cost(&self, data: &T) -> usize;
}

impl<T, F> CacheCostEvaluator<T> for F
where
    F: Fn(&T) -> usize,
{
    fn eval_cost(&self, data: &T) -> usize {
        self(data)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct CacheKey(*const ());

impl CacheKey {
    fn new<T>(data: &Arc<T>) -> Self {
        CacheKey(Arc::as_ptr(data).cast::<()>())
    }
}

mod cache_entry {
    use std::sync::atomic::AtomicUsize;

    pub(super) struct CacheEntry<T> {
        data: T,
        cost: usize,
        cache_ref_count: AtomicUsize,
    }

    impl<T> CacheEntry<T> {
        pub(super) fn new(data: T, cost: usize) -> Self {
            CacheEntry {
                data,
                cost,
                cache_ref_count: AtomicUsize::new(0),
            }
        }

        pub(super) fn data(&self) -> &T {
            &self.data
        }

        pub(super) fn cost(&self) -> usize {
            self.cost
        }

        pub(super) fn increment_ref_count(&self) {
            self.cache_ref_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }

        pub(super) fn decrement_ref_count(&self) -> bool {
            let count = self
                .cache_ref_count
                .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            count == 1
        }
    }
}

mod config {
    pub(super) struct StoreConfig<T> {
        max_cost: usize,
        cost_eval: Box<dyn super::CacheCostEvaluator<T>>,
    }

    impl<T> StoreConfig<T> {
        pub(super) fn new<E>(max_cost: usize, eval: E) -> Self
        where
            E: super::CacheCostEvaluator<T> + Sync + 'static,
        {
            StoreConfig {
                max_cost,
                cost_eval: Box::new(eval),
            }
        }

        pub(super) fn max_cost(&self) -> usize {
            self.max_cost
        }

        pub(super) fn eval_cost(&self, data: &T) -> usize {
            self.cost_eval.eval_cost(data)
        }
    }
}

mod store_mut {
    use std::{collections::HashMap, sync::Arc};

    use super::{CacheEntry, CacheKey, StoreConfig};

    pub(super) struct StoreMut<T> {
        curr_cost: usize,
        cached_values: HashMap<CacheKey, Arc<super::CacheEntry<T>>>,
    }

    impl<T> StoreMut<T> {
        pub(super) fn new() -> Self {
            StoreMut {
                curr_cost: 0,
                cached_values: HashMap::new(),
            }
        }

        pub(super) fn allocate(&mut self, config: &StoreConfig<T>, data: T) -> Arc<CacheEntry<T>> {
            let cost = config.eval_cost(&data);
            if cost + self.curr_cost > config.max_cost() {
                self.evict(config, cost);
            }
            assert!(cost + self.curr_cost <= config.max_cost());
            let entry = Arc::new(CacheEntry::new(data, cost));
            self.cached_values
                .insert(CacheKey::new(&entry), entry.clone());
            self.curr_cost += cost;
            entry
        }

        pub(super) fn evict_key(&mut self, key: CacheKey) {
            if let Some(entry) = self.cached_values.remove(&key) {
                self.curr_cost -= entry.cost();
            }
        }

        fn evict(&mut self, config: &StoreConfig<T>, cost: usize) {
            // Dumb for now: Just evict any entry in the cache
            while cost + self.curr_cost > config.max_cost() {
                if let Some(key) = self.cached_values.keys().next().copied() {
                    self.evict_key(key);
                } else {
                    break;
                }
            }
        }
    }
}

mod inner {
    use std::sync::{Arc, Mutex};

    use super::{CacheCostEvaluator, CacheEntry, StoreConfig, StoreMut};
    pub(super) struct StoreInner<T> {
        config: StoreConfig<T>,
        inner: Mutex<StoreMut<T>>,
    }

    impl<T> StoreInner<T> {
        pub(super) fn new<E>(max_size: usize, eval: E) -> Self
        where
            E: CacheCostEvaluator<T> + Sync + 'static,
        {
            Self {
                config: StoreConfig::new(max_size, eval),
                inner: Mutex::new(StoreMut::new()),
            }
        }

        pub(super) fn allocate(&self, data: T) -> Arc<CacheEntry<T>> {
            let mut guard = self.inner.lock().unwrap();
            guard.allocate(&self.config, data)
        }

        pub(super) fn lock(&self) -> std::sync::MutexGuard<'_, StoreMut<T>> {
            self.inner.lock().unwrap()
        }
    }
}

use cache_entry::CacheEntry;
use config::StoreConfig;
use inner::StoreInner;
use store_mut::StoreMut;

pub struct CacheStore<T>(Arc<StoreInner<T>>);

impl<T> CacheStore<T> {
    pub fn new<F>(max_size: usize, cost_eval: F) -> Self
    where
        F: Fn(&T) -> usize + Sync + 'static,
    {
        Self(Arc::new(StoreInner::new(max_size, cost_eval)))
    }

    pub fn insert(&self, data: T) -> CacheRef<T> {
        let cache_entry = self.0.allocate(data);
        CacheRef::new(&self.0, &cache_entry)
    }
}

pub struct CacheRef<T> {
    store: Weak<StoreInner<T>>,
    entry: Weak<CacheEntry<T>>,
}

impl<T> CacheRef<T> {
    fn new(store: &Arc<StoreInner<T>>, entry: &Arc<CacheEntry<T>>) -> Self {
        entry.increment_ref_count();
        CacheRef {
            store: Arc::downgrade(store),
            entry: Arc::downgrade(entry),
        }
    }

    #[must_use]
    pub fn lock(&self) -> Option<Guard<'_, T>> {
        self.entry.upgrade().map(|e| Guard {
            entry: e,
            _marker: std::marker::PhantomData,
        })
    }
}

impl<T> Clone for CacheRef<T> {
    fn clone(&self) -> Self {
        if let (Some(store), Some(entry)) = (self.store.upgrade(), self.entry.upgrade()) {
            Self::new(&store, &entry)
        } else {
            CacheRef {
                store: Weak::new(),
                entry: Weak::new(),
            }
        }
    }
}

impl<T> Drop for CacheRef<T> {
    fn drop(&mut self) {
        if let Some(entry) = self.entry.upgrade() {
            if entry.decrement_ref_count() {
                // Entry is no longer referenced, we can remove it from the store
                if let Some(store) = self.store.upgrade() {
                    let mut guard = store.lock();
                    guard.evict_key(CacheKey::new(&entry));
                }
            }
        }
    }
}

pub struct Guard<'a, T> {
    entry: Arc<CacheEntry<T>>,
    _marker: std::marker::PhantomData<&'a ()>,
}

impl<T> Deref for Guard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.entry.data()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_store() {
        let store = CacheStore::new(100, |data: &Vec<u8>| data.len());
        let data = vec![1, 2, 3, 4];
        let cache_ref = store.insert(data.clone());
        assert_eq!(*cache_ref.lock().unwrap(), data);
    }

    #[test]
    fn test_cache_evict() {
        let store = CacheStore::new(4, |data: &Vec<u8>| data.len());
        let data1 = vec![1, 2];
        let data2 = vec![3, 4];
        let _cache_ref1 = store.insert(data1);
        let _cache_ref2 = store.insert(data2);
    }
}
