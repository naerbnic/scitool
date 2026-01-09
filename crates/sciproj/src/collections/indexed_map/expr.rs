//! Expressions, which can be used to query the map.

use crate::collections::indexed_map::{ReadMapStorageGuard, StorageId};

pub(crate) struct Predicate<'a, T> {
    pred: Box<dyn for<'item> Fn(&'item ReadMapStorageGuard<'item, T>, StorageId) -> bool + 'a>,
}
