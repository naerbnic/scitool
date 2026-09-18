//! [`ProjArc`] provides the ability to work with references to components of an
//! `Arc<T>` that share the same allocation, and do not require heap allocations.
#![expect(unsafe_code)]

use std::{
    any::Any,
    borrow::Borrow,
    fmt::{Debug, Display},
    hash::{Hash, Hasher},
    ops::Deref,
    sync::{Arc, Weak},
};

pub(crate) struct ProjArc<T> {
    any_arc: Arc<dyn Any + Send + Sync>,
    ptr: *const T,
}

impl<T> ProjArc<T>
where
    T: Send + Sync + 'static,
{
    pub(crate) fn new(val: T) -> Self {
        Self::from_arc(Arc::new(val))
    }

    pub(crate) fn from_arc(arc: Arc<T>) -> Self {
        let ptr = std::ptr::from_ref(arc.as_ref());
        let any_arc: Arc<dyn Any + Send + Sync> = arc;
        Self { any_arc, ptr }
    }

    pub(crate) fn map<F, R>(this: &Self, mapper: F) -> ProjArc<R>
    where
        for<'a> F: FnOnce(&'a T) -> &'a R,
        R: Send + Sync + 'static,
    {
        // SAFETY: We know that the reference we obtain from the mapper has
        // the same lifetime as the one from the input. That lifetime is
        // dynamically connected to the strong count of the internal Arc. As
        // long as we can ensure the strong count doesn't drop to 0, it is
        // safe to use the pointer.
        let new_ptr: *const R = mapper(unsafe { &*this.ptr });
        ProjArc {
            any_arc: this.any_arc.clone(),
            ptr: new_ptr,
        }
    }

    #[expect(dead_code)]
    pub(crate) fn downgrade(self: &ProjArc<T>) -> WeakProjArc<T> {
        let any_weak = Arc::downgrade(&self.any_arc);
        WeakProjArc {
            any_weak,
            ptr: self.ptr,
        }
    }
}

impl<T> Deref for ProjArc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}

impl<T> AsRef<T> for ProjArc<T> {
    fn as_ref(&self) -> &T {
        self
    }
}

impl<T> Borrow<T> for ProjArc<T> {
    fn borrow(&self) -> &T {
        self
    }
}

impl<T> PartialEq for ProjArc<T>
where
    T: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}

impl<T> Eq for ProjArc<T> where T: Eq {}

impl<T> PartialOrd for ProjArc<T>
where
    T: PartialOrd,
{
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        (**self).partial_cmp(&**other)
    }
}

impl<T> Ord for ProjArc<T>
where
    T: Ord,
{
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (**self).cmp(&**other)
    }
}

impl<T> Hash for ProjArc<T>
where
    T: Hash,
{
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        (**self).hash(state);
    }
}

impl<T> Default for ProjArc<T>
where
    T: Default + Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> Debug for ProjArc<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ProjArc").field(&**self).finish()
    }
}

impl<T> Display for ProjArc<T>
where
    T: Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        (**self).fmt(f)
    }
}

unsafe impl<T> Send for ProjArc<T> where T: Send + Sync {}
unsafe impl<T> Sync for ProjArc<T> where T: Send + Sync {}

impl<T> Clone for ProjArc<T> {
    fn clone(&self) -> Self {
        Self {
            any_arc: self.any_arc.clone(),
            ptr: self.ptr,
        }
    }
}

pub(crate) struct WeakProjArc<T> {
    any_weak: Weak<dyn Any + Send + Sync>,
    /// This pointer is never null. If the `any_weak` contents are dropped, it
    /// may point to unallocated/unsafe memory. This must be checked during
    /// upgrade.
    ptr: *const T,
}

impl<T> WeakProjArc<T> {
    #[expect(dead_code)]
    pub(crate) fn upgrade(&self) -> Option<ProjArc<T>> {
        let any_arc = self.any_weak.upgrade()?;
        Some(ProjArc {
            any_arc,
            ptr: self.ptr,
        })
    }
}

impl<T> Clone for WeakProjArc<T> {
    fn clone(&self) -> Self {
        Self {
            any_weak: self.any_weak.clone(),
            ptr: self.ptr,
        }
    }
}

unsafe impl<T> Send for WeakProjArc<T> where T: Send + Sync {}
unsafe impl<T> Sync for WeakProjArc<T> where T: Send + Sync {}
