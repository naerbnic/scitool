use std::sync::{Arc, Mutex, OnceLock};

use crate::event::{EventTrigger, EventWaiter};

type Callback<T> = Box<dyn FnOnce(&T) + Send + 'static>;

struct SharedState<T> {
    data: OnceLock<T>,
    callbacks: Mutex<Option<Vec<Callback<T>>>>,
}

pub(crate) struct Promise<T> {
    shared: Arc<SharedState<T>>,
    event: EventWaiter,
}

impl<T> Promise<T>
where
    T: Sync,
{
    pub(super) fn new() -> (Self, Fulfiller<T>) {
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
            Fulfiller {
                shared,
                event: trigger,
            },
        )
    }

    pub(super) fn new_ready(value: T) -> Self {
        let shared = Arc::new(SharedState {
            data: OnceLock::from(value),
            callbacks: Mutex::new(None),
        });
        Self {
            shared: shared.clone(),
            event: EventWaiter::new_ready(),
        }
    }

    pub(crate) fn try_get(&self) -> Option<&T> {
        self.shared.data.get()
    }

    pub(crate) fn get_sync(&self) -> &T {
        self.try_get().unwrap_or_else(|| {
            self.event.wait_sync().unwrap();
            self.try_get().unwrap()
        })
    }

    pub(crate) async fn get_async(&self) -> &T {
        if let Some(curr_ref) = self.try_get() {
            return curr_ref;
        }
        self.event.wait_async().await.unwrap();
        self.try_get().unwrap()
    }

    pub(crate) fn map<F, R>(&self, func: F) -> Promise<R>
    where
        F: FnOnce(&T) -> R + Send + 'static,
        R: Send + Sync + 'static,
    {
        // Initial check for relative speed (no need to wait on locks)
        if let Some(value) = self.try_get() {
            return Promise::new_ready(func(value));
        }

        let mut guard = self.shared.callbacks.lock().unwrap();

        let Some(callbacks) = &mut *guard else {
            // The value has already been set.
            drop(guard);
            return Promise::new_ready(func(self.try_get().unwrap()));
        };
        let (promise, fulfiller) = Promise::new();

        callbacks.push(Box::new(move |value| {
            fulfiller.fulfill(func(value));
        }));
        promise
    }
}

impl<T> Clone for Promise<T> {
    fn clone(&self) -> Self {
        Self {
            shared: self.shared.clone(),
            event: self.event.clone(),
        }
    }
}

pub(crate) struct Fulfiller<T> {
    shared: Arc<SharedState<T>>,
    event: EventTrigger,
}

impl<T> Fulfiller<T> {
    pub(crate) fn fulfill(mut self, value: T) {
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

impl<T> Drop for Fulfiller<T> {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.shared.callbacks.lock() {
            let _unused = guard.take();
        }
    }
}
