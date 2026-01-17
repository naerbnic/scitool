//! Expressions, which can be used to query the map.

use crate::collections::indexed_map::{MapStorage, StorageId};

pub(crate) struct Predicate<'a, T> {
    pred: Box<dyn for<'item> Fn(&'item MapStorage<T>, StorageId) -> bool + 'a>,
}
