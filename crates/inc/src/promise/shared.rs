use std::{
    any::TypeId,
    marker::PhantomData,
    ptr,
    sync::{Arc, Mutex, OnceLock},
};

use crate::{
    event::{EventTrigger, EventWaiter},
    types::{AnyArc, AnyValue},
};

type Callback = Box<dyn FnOnce(&AnyValue) + Send + 'static>;

struct SharedState {
    data: OnceLock<AnyArc>,
    type_id: TypeId,
    callbacks: Mutex<Option<Vec<Callback>>>,
}

struct ErasedPromise {
    shared: Arc<SharedState>,
    event: EventWaiter,
}

impl ErasedPromise {
    fn new(type_id: TypeId) -> (Self, ErasedFulfiller) {
        let (trigger, waiter) = crate::event::new_event_pair();
        let shared = Arc::new(SharedState {
            data: OnceLock::new(),
            type_id,
            callbacks: Mutex::new(Some(Vec::new())),
        });
        (
            Self {
                shared: shared.clone(),
                event: waiter,
            },
            ErasedFulfiller {
                shared,
                event: trigger,
            },
        )
    }

    fn new_ready<T>(value: Arc<T>) -> Self
    where
        T: Send + Sync + 'static,
    {
        let value: AnyArc = value;
        let shared = Arc::new(SharedState {
            data: OnceLock::from(value),
            type_id: TypeId::of::<T>(),
            callbacks: Mutex::new(None),
        });
        Self {
            shared: shared.clone(),
            event: EventWaiter::new_ready(),
        }
    }

    fn try_get(&self) -> Option<&AnyArc> {
        self.shared.data.get()
    }

    fn get_sync(&self) -> &AnyArc {
        self.try_get().unwrap_or_else(|| {
            self.event.wait().wait_sync();
            self.try_get().unwrap()
        })
    }

    async fn get_async(&self) -> &AnyArc {
        if let Some(curr_ref) = self.try_get() {
            return curr_ref;
        }
        self.event.wait().await;
        self.try_get().unwrap()
    }

    fn map<F, R>(&self, func: F) -> ErasedPromise
    where
        F: FnOnce(&AnyValue) -> Arc<R> + Send + 'static,
        R: Send + Sync + 'static,
    {
        // Initial check for relative speed (no need to wait on locks)
        if let Some(value) = self.try_get() {
            return ErasedPromise::new_ready(func(value));
        }

        let mut guard = self.shared.callbacks.lock().unwrap();

        let Some(callbacks) = &mut *guard else {
            // The value has already been set.
            drop(guard);
            return ErasedPromise::new_ready(func(self.try_get().unwrap()));
        };
        let (promise, fulfiller) = ErasedPromise::new(TypeId::of::<R>());

        callbacks.push(Box::new(move |value| {
            fulfiller.fulfill(func(value));
        }));
        promise
    }
}

impl Clone for ErasedPromise {
    fn clone(&self) -> Self {
        Self {
            shared: self.shared.clone(),
            event: self.event.clone(),
        }
    }
}

struct ErasedFulfiller {
    shared: Arc<SharedState>,
    event: EventTrigger,
}

impl ErasedFulfiller {
    fn fulfill(mut self, value: AnyArc) {
        if self.shared.data.set(value).is_err() {
            unreachable!("At most once ensured by ownership");
        }

        let Some(callbacks) = std::mem::take(&mut *self.shared.callbacks.lock().unwrap()) else {
            unreachable!("Only the fulfill method removes the vector")
        };

        self.event.trigger();
        let Some(data_ref) = self.shared.data.get() else {
            unreachable!("Enforced by set")
        };
        for callback in callbacks {
            callback(data_ref);
        }
    }
}

impl Drop for ErasedFulfiller {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.shared.callbacks.lock() {
            let _unused = guard.take();
        }
    }
}

// Uses repr transparent to allow downcast_ref() to be sound.
#[repr(transparent)]
pub struct SharedPromise<T> {
    inner: ErasedPromise,
    _phantom: PhantomData<T>,
}

impl<T> SharedPromise<T>
where
    T: Send + Sync + 'static,
{
    #[must_use]
    pub(crate) fn new() -> (Self, SharedFulfiller<T>) {
        let (any_promise, any_fulfiller) = ErasedPromise::new(TypeId::of::<T>());
        (
            Self {
                inner: any_promise,
                _phantom: PhantomData,
            },
            SharedFulfiller {
                inner: any_fulfiller,
                _phantom: PhantomData,
            },
        )
    }

    #[must_use]
    pub fn new_ready(value: Arc<T>) -> Self {
        Self {
            inner: ErasedPromise::new_ready(value),
            _phantom: PhantomData,
        }
    }

    pub(crate) fn try_get(&self) -> Option<&T> {
        self.inner.try_get().and_then(|v| v.downcast_ref())
    }

    #[expect(dead_code)]
    pub(crate) fn get_sync(&self) -> &T {
        self.inner.get_sync().downcast_ref().unwrap()
    }

    #[expect(dead_code)]
    pub(crate) fn get_sync_arc(&self) -> Arc<T> {
        Arc::downcast(self.inner.get_sync().clone()).ok().unwrap()
    }

    #[expect(dead_code)]
    pub(crate) async fn get_async(&self) -> &T {
        self.inner.get_async().await.downcast_ref().unwrap()
    }

    #[expect(dead_code)]
    pub(crate) async fn get_async_arc(&self) -> Arc<T> {
        Arc::downcast(self.inner.get_async().await.clone())
            .ok()
            .unwrap()
    }

    #[expect(dead_code)]
    pub(crate) fn map<F, R>(&self, func: F) -> SharedPromise<R>
    where
        F: FnOnce(&T) -> R + Send + 'static,
        R: Send + Sync + 'static,
    {
        self.map_arc(move |v| Arc::new(func(v)))
    }

    #[expect(dead_code)]
    pub(crate) fn map_arc<F, R>(&self, func: F) -> SharedPromise<R>
    where
        F: FnOnce(&T) -> Arc<R> + Send + 'static,
        R: Send + Sync + 'static,
    {
        let inner = self.inner.map(move |v| {
            let Some(cast_ref) = v.downcast_ref() else {
                unreachable!("Prevented by type system during construction")
            };
            func(cast_ref)
        });
        SharedPromise {
            inner,
            _phantom: PhantomData,
        }
    }

    pub(crate) fn as_any(&self) -> &SharedAnyPromise {
        // SAFETY: AnyPromise and Promise are repr(transparent) over ErasedPromise.
        unsafe { &*ptr::from_ref::<Self>(self).cast::<SharedAnyPromise>() }
    }

    pub(crate) fn into_any(self) -> SharedAnyPromise {
        SharedAnyPromise { inner: self.inner }
    }
}

impl<T> Clone for SharedPromise<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            _phantom: PhantomData,
        }
    }
}

pub(crate) struct SharedFulfiller<T> {
    inner: ErasedFulfiller,
    _phantom: PhantomData<fn(T)>,
}

impl<T> SharedFulfiller<T>
where
    T: Send + Sync + 'static,
{
    pub(crate) fn fulfill_arc(self, value: Arc<T>) {
        self.inner.fulfill(value);
    }

    pub(crate) fn fulfill(self, value: T) {
        self.inner.fulfill(Arc::new(value));
    }
}

#[repr(transparent)]
pub(crate) struct SharedAnyPromise {
    inner: ErasedPromise,
}

impl SharedAnyPromise {
    #[allow(unsafe_code)]
    pub(crate) fn downcast_ref<T>(&self) -> Option<&SharedPromise<T>>
    where
        T: Send + Sync + 'static,
    {
        if self.inner.shared.type_id != TypeId::of::<T>() {
            return None;
        }
        // SAFETY: Promise<T> is repr(transparent) over ErasedPromise, so the layout of the two
        // is identical.
        unsafe { Some(&*(&raw const self.inner).cast::<SharedPromise<T>>()) }
    }

    pub(crate) fn get_sync_arc(&self) -> &AnyArc {
        self.inner.get_sync()
    }

    pub(crate) async fn get_async_arc(&self) -> &AnyArc {
        self.inner.get_async().await
    }
}

impl Clone for SharedAnyPromise {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}
