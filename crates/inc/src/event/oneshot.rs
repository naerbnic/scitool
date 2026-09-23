use std::{collections::HashMap, fmt::Debug, num::NonZeroUsize, pin::Pin, sync::Mutex};

/// An object representing a state that can be activated at a specific time.
pub(crate) struct Event(Mutex<EventState>);

impl Debug for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Event").finish()
    }
}

impl Event {
    pub(crate) fn new() -> Self {
        Self(Mutex::new(EventState::Pending {
            next_id: NonZeroUsize::new(1).unwrap(),
            waiters: HashMap::new(),
            callbacks: Vec::new(),
        }))
    }

    pub(crate) fn new_signaled() -> Self {
        Self(Mutex::new(EventState::Signaled))
    }

    pub(crate) fn wait_sync(&self) {
        futures_executor::block_on(self.wait_async());
    }

    pub(crate) fn wait_async(&self) -> EventFuture<'_> {
        EventFuture {
            event: self,
            key: None,
        }
    }

    pub(crate) fn is_pending(&self) -> bool {
        let guard = self.0.lock().unwrap();
        matches!(*guard, EventState::Pending { .. })
    }

    fn add_waiter<F>(&self, key: &mut Option<NonZeroUsize>, waiter_fn: F) -> bool
    where
        F: FnOnce() -> std::task::Waker,
    {
        let mut guard = self.0.lock().unwrap();
        let EventState::Pending {
            next_id, waiters, ..
        } = &mut *guard
        else {
            return false;
        };

        let id = *next_id;
        let next_val = next_id.get().wrapping_add(1);
        *next_id = NonZeroUsize::new(if next_val == 0 { 1 } else { next_val }).unwrap();
        waiters.insert(id, waiter_fn());
        *key = Some(id);
        true
    }

    #[expect(dead_code)]
    pub(super) fn add_signal_callback<F>(&self, callback: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let mut guard = self.0.lock().unwrap();
        let EventState::Pending { callbacks, .. } = &mut *guard else {
            // We don't want to hold the lock across the callback.
            drop(guard);
            callback();
            return;
        };
        callbacks.push(Box::new(callback));
    }

    pub(super) fn drop_callbacks(&self) {
        let mut guard = self.0.lock().unwrap();
        let EventState::Pending { callbacks, .. } = &mut *guard else {
            return;
        };
        callbacks.clear();
    }

    fn try_update_waker(&self, key: NonZeroUsize, cx_waker: &std::task::Waker) -> bool {
        let mut guard = self.0.lock().unwrap();
        let EventState::Pending { waiters, .. } = &mut *guard else {
            return false;
        };

        if let Some(waker) = waiters.get_mut(&key) {
            if !waker.will_wake(cx_waker) {
                waker.clone_from(cx_waker);
            }
            return true;
        }
        false
    }

    fn remove_waiter(&self, key: NonZeroUsize) {
        let mut guard = self.0.lock().unwrap();
        if let EventState::Pending { waiters, .. } = &mut *guard {
            waiters.remove(&key);
        }
    }

    /// Cause the event to be signaled.
    pub(crate) fn signal(&self) {
        let callbacks = {
            let mut guard = self.0.lock().unwrap();
            let EventState::Pending {
                waiters, callbacks, ..
            } = std::mem::replace(&mut *guard, EventState::Signaled)
            else {
                return;
            };

            #[allow(clippy::iter_over_hash_type)]
            for (_, waiter) in waiters {
                waiter.wake();
            }
            callbacks
        };

        for callback in callbacks {
            callback();
        }
    }
}

pub(crate) struct EventFuture<'a> {
    event: &'a Event,
    key: Option<NonZeroUsize>,
}

impl std::future::Future for EventFuture<'_> {
    type Output = ();

    fn poll(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.get_mut();
        if let Some(key) = this.key {
            if this.event.try_update_waker(key, cx.waker()) {
                std::task::Poll::Pending
            } else {
                std::task::Poll::Ready(())
            }
        } else if this.event.add_waiter(&mut this.key, || cx.waker().clone()) {
            std::task::Poll::Pending
        } else {
            std::task::Poll::Ready(())
        }
    }
}

impl Drop for EventFuture<'_> {
    fn drop(&mut self) {
        if let Some(key) = self.key {
            self.event.remove_waiter(key);
        }
    }
}

enum EventState {
    Pending {
        next_id: NonZeroUsize,
        waiters: HashMap<NonZeroUsize, std::task::Waker>,
        callbacks: Vec<Box<dyn FnOnce() + Send + 'static>>,
    },
    Signaled,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_future_drop_cleans_up_waiter() {
        let event = Event::new();
        assert!(event.is_pending());

        {
            let mut fut = std::pin::pin!(event.wait_async());
            let waker = std::task::Waker::noop();
            let mut cx = std::task::Context::from_waker(waker);

            assert_eq!(fut.as_mut().poll(&mut cx), std::task::Poll::Pending);

            // Verify a waiter was registered
            let guard = event.0.lock().unwrap();
            let EventState::Pending { waiters, .. } = &*guard else {
                panic!("expected pending");
            };
            assert_eq!(waiters.len(), 1);
        }

        // Dropping the future should have removed the waiter from event
        let guard = event.0.lock().unwrap();
        let EventState::Pending { waiters, .. } = &*guard else {
            panic!("expected pending");
        };
        assert_eq!(waiters.len(), 0);
    }
}
