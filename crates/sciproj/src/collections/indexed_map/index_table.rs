use std::{any::Any, collections::HashMap, fmt::Debug, marker::PhantomData};

use super::{index::ManagedIndex, unique_token::UniqueToken};

pub(super) struct IndexId<I> {
    token: UniqueToken,
    _phantom: PhantomData<fn() -> I>,
}

impl<I> Debug for IndexId<I> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IndexId")
            .field("token", &self.token)
            .finish()
    }
}

impl<I> Clone for IndexId<I> {
    fn clone(&self) -> Self {
        Self {
            token: self.token.clone(),
            _phantom: PhantomData,
        }
    }
}

impl<I> PartialEq for IndexId<I> {
    fn eq(&self, other: &Self) -> bool {
        self.token == other.token
    }
}

impl<I> Eq for IndexId<I> {}
/// A table of index objects. This is necessary to allow the index objects to
/// be !Send and !Sync, while still allowing the index to be used from
/// multiple threads.
pub(super) struct IndexTable<T> {
    indices: HashMap<UniqueToken, Box<dyn ManagedIndex<T>>>,
}

impl<T> IndexTable<T>
where
    T: 'static,
{
    pub(super) fn new() -> Self {
        Self {
            indices: HashMap::new(),
        }
    }

    pub(super) fn insert<I>(&mut self, index: I) -> IndexId<I>
    where
        I: ManagedIndex<T>,
    {
        let token = UniqueToken::new();
        self.indices.insert(token.clone(), Box::new(index));
        IndexId {
            token,
            _phantom: PhantomData,
        }
    }

    pub(super) fn get<I>(&self, id: &IndexId<I>) -> Option<&I>
    where
        I: ManagedIndex<T>,
    {
        let index = self.indices.get(&id.token)?;
        let index: &dyn Any = &**index;
        Some(
            index
                .downcast_ref::<I>()
                .expect("Type system should enforce."),
        )
    }

    pub(super) fn get_mut<I>(&mut self, id: &IndexId<I>) -> Option<&mut I>
    where
        I: ManagedIndex<T> + 'static,
    {
        let index = self.indices.get_mut(&id.token)?;
        let index: &mut dyn Any = &mut **index;
        Some(
            index
                .downcast_mut::<I>()
                .expect("Type system should enforce."),
        )
    }

    pub(super) fn values_mut(&mut self) -> impl Iterator<Item = &mut dyn ManagedIndex<T>> {
        self.indices.values_mut().map(|index| &mut **index)
    }
}
