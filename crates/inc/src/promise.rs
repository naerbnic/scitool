use std::{
    any::{Any, TypeId},
    marker::PhantomData,
    sync::{Arc, Mutex, OnceLock},
};

use crate::event::{EventTrigger, EventWaiter};

type AnyValue = dyn Any + Send + Sync + 'static;

type AnyArc = Arc<AnyValue>;

type Callback = Box<dyn FnOnce(&AnyValue) + Send + 'static>;

struct SharedState {
    data: OnceLock<AnyArc>,
    callbacks: Mutex<Option<Vec<Callback>>>,
}

struct ErasedPromise {
    shared: Arc<SharedState>,
    event: EventWaiter,
}

impl ErasedPromise {
    fn new() -> (Self, ErasedFulfiller) {
        let (trigger, waiter) = crate::event::new_event_pair();
        let shared = Arc::new(SharedState {
            data: OnceLock::new(),
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
            self.event.wait_sync().unwrap();
            self.try_get().unwrap()
        })
    }

    async fn get_async(&self) -> &AnyArc {
        if let Some(curr_ref) = self.try_get() {
            return curr_ref;
        }
        self.event.wait_async().await.unwrap();
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
        let (promise, fulfiller) = ErasedPromise::new();

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
pub(crate) struct Promise<T> {
    inner: ErasedPromise,
    _phantom: PhantomData<T>,
}

impl<T> Promise<T>
where
    T: Send + Sync + 'static,
{
    #[must_use]
    pub(crate) fn new() -> (Self, Fulfiller<T>) {
        let (any_promise, any_fulfiller) = ErasedPromise::new();
        (
            Self {
                inner: any_promise,
                _phantom: PhantomData,
            },
            Fulfiller {
                inner: any_fulfiller,
                _phantom: PhantomData,
            },
        )
    }

    #[must_use]
    pub(crate) fn new_ready(value: Arc<T>) -> Self {
        Self {
            inner: ErasedPromise::new_ready(value),
            _phantom: PhantomData,
        }
    }

    pub(crate) fn try_get(&self) -> Option<&T> {
        self.inner.try_get().and_then(|v| v.downcast_ref())
    }

    pub(crate) fn try_get_arc(&self) -> Option<Arc<T>> {
        self.inner
            .try_get()
            .map(|v| Arc::downcast(v.clone()).ok().unwrap())
    }

    pub(crate) fn get_sync(&self) -> &T {
        self.inner.get_sync().downcast_ref().unwrap()
    }

    pub(crate) fn get_sync_arc(&self) -> Arc<T> {
        Arc::downcast(self.inner.get_sync().clone()).ok().unwrap()
    }

    pub(crate) async fn get_async(&self) -> &T {
        self.inner.get_async().await.downcast_ref().unwrap()
    }

    pub(crate) async fn get_async_arc(&self) -> Arc<T> {
        Arc::downcast(self.inner.get_async().await.clone())
            .ok()
            .unwrap()
    }

    pub(crate) fn map<F, R>(&self, func: F) -> Promise<R>
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
        Promise {
            inner,
            _phantom: PhantomData,
        }
    }

    pub(crate) fn to_any(&self) -> AnyPromise {
        AnyPromise {
            inner: self.inner.clone(),
            type_id: TypeId::of::<T>(),
        }
    }

    pub(crate) fn into_any(self) -> AnyPromise {
        AnyPromise {
            inner: self.inner,
            type_id: TypeId::of::<T>(),
        }
    }
}

impl<T> Clone for Promise<T>
where
    T: Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            _phantom: PhantomData,
        }
    }
}

pub(crate) struct Fulfiller<T> {
    inner: ErasedFulfiller,
    _phantom: PhantomData<fn(T)>,
}

impl<T> Fulfiller<T>
where
    T: Send + Sync + 'static,
{
    pub(crate) fn fulfill(self, value: Arc<T>) {
        self.inner.fulfill(value);
    }
}

pub(crate) struct AnyPromise {
    inner: ErasedPromise,
    type_id: TypeId,
}

impl AnyPromise {
    pub(crate) fn downcast<T>(self) -> Result<Promise<T>, Self>
    where
        T: Send + Sync + 'static,
    {
        if self.type_id != TypeId::of::<T>() {
            return Err(self);
        }
        Ok(Promise {
            inner: self.inner,
            _phantom: PhantomData,
        })
    }

    #[allow(unsafe_code)]
    pub(crate) fn downcast_ref<T>(&self) -> Option<&Promise<T>>
    where
        T: Send + Sync + 'static,
    {
        if self.type_id != TypeId::of::<T>() {
            return None;
        }
        // SAFETY: Promise<T> is repr(transparent) over ErasedPromise, so the layout of the two
        // is identical.
        unsafe { Some(&*(&raw const self.inner).cast::<Promise<T>>()) }
    }

    pub(crate) fn wait_sync(&self) {
        let _ = self.inner.get_sync();
    }

    pub(crate) async fn wait_async(&self) {
        let _ = self.inner.get_async().await;
    }
}

impl Clone for AnyPromise {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            type_id: self.type_id,
        }
    }
}
