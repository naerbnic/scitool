use std::any::Any;

use super::storage::{MapStorage, StorageId};

#[derive(Debug, thiserror::Error)]
#[error("The key already exists.")]
pub(super) struct IndexInsertError;

pub(super) trait ManagedIndex<T>: Any {
    /// If this is a unique index, and it already has another element that conflicts,
    /// return the `StorageId` of the existing element.
    fn get_conflict(&self, storage: &MapStorage<T>, id: &T) -> Option<StorageId>;
    fn insert(&mut self, storage: &MapStorage<T>, id: StorageId) -> Result<(), IndexInsertError>;
    fn remove(&mut self, storage: &MapStorage<T>, id: StorageId);
}
