use std::any::Any;

use super::storage::{MapStorage, StorageId};

pub(super) trait ManagedIndex<T>: Any {
    fn insert(&mut self, storage: &MapStorage<T>, id: StorageId);
    fn remove(&mut self, storage: &MapStorage<T>, id: StorageId);
}
