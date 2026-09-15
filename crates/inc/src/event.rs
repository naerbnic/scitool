use std::{
    borrow::Borrow,
    collections::HashMap,
    fmt::Debug,
    num::NonZeroUsize,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

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
        }))
    }

    pub(crate) fn wait_sync(&self) {
        let mut key = None;
        if !self.add_waiter(&mut key, Waiter::new_thread) {
            return;
        }

        // Block until signaled. Park can spuriously wake up, so we have to
        // do this in a loop.
        loop {
            std::thread::park();

            if !self.is_pending() {
                return;
            }
        }
    }

    pub(crate) fn wait_async(&self) -> impl Future<Output = ()> {
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
        F: FnOnce() -> Waiter,
    {
        let mut guard = self.0.lock().unwrap();
        let EventState::Pending { next_id, waiters } = &mut *guard else {
            return false;
        };

        let id = *next_id;
        let next_val = next_id.get().wrapping_add(1);
        *next_id = NonZeroUsize::new(if next_val == 0 { 1 } else { next_val }).unwrap();
        waiters.insert(id, waiter_fn());
        *key = Some(id);
        true
    }

    fn try_update_waker(&self, key: NonZeroUsize, cx_waker: &std::task::Waker) -> bool {
        let mut guard = self.0.lock().unwrap();
        let EventState::Pending { waiters, .. } = &mut *guard else {
            return false;
        };

        if let Some(Waiter::Async(existing_waker)) = waiters.get_mut(&key) {
            if !existing_waker.will_wake(cx_waker) {
                *existing_waker = cx_waker.clone();
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
        let mut guard = self.0.lock().unwrap();
        let EventState::Pending { waiters, .. } =
            std::mem::replace(&mut *guard, EventState::Signaled)
        else {
            return;
        };

        for (_, waiter) in waiters {
            waiter.wake();
        }
    }
}

struct EventFuture<E: Borrow<Event>> {
    event: E,
    key: Option<NonZeroUsize>,
}

impl<E> std::future::Future for EventFuture<E>
where
    E: Borrow<Event> + Unpin,
{
    type Output = ();

    fn poll(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.get_mut();
        if let Some(key) = this.key {
            if this.event.borrow().try_update_waker(key, cx.waker()) {
                std::task::Poll::Pending
            } else {
                std::task::Poll::Ready(())
            }
        } else if this
            .event
            .borrow()
            .add_waiter(&mut this.key, || Waiter::new_waker(cx.waker().clone()))
        {
            std::task::Poll::Pending
        } else {
            std::task::Poll::Ready(())
        }
    }
}

impl<E> Drop for EventFuture<E>
where
    E: Borrow<Event>,
{
    fn drop(&mut self) {
        if let Some(key) = self.key {
            self.event.borrow().remove_waiter(key);
        }
    }
}

enum Waiter {
    Async(std::task::Waker),
    Sync(std::thread::Thread),
}

impl Waiter {
    fn new_thread() -> Self {
        Self::Sync(std::thread::current())
    }

    fn new_waker(waker: std::task::Waker) -> Self {
        Self::Async(waker)
    }

    fn wake(self) {
        match self {
            Waiter::Async(waker) => waker.wake(),
            Waiter::Sync(thread) => thread.unpark(),
        }
    }
}

enum EventState {
    Pending {
        next_id: NonZeroUsize,
        waiters: HashMap<NonZeroUsize, Waiter>,
    },
    Signaled,
}

struct PairInner {
    event: Event,
    was_cancelled: AtomicBool,
}

pub(crate) struct EventTrigger(Option<Arc<PairInner>>);

impl EventTrigger {
    pub(crate) fn trigger(mut self) {
        self.0.take().unwrap().event.signal();
    }
}

impl Drop for EventTrigger {
    fn drop(&mut self) {
        if let Some(inner) = self.0.take() {
            // We did not trigger this event, so we have to indicate it is cancelled.
            // Only we have the ability to signal, so there's no chance of a race
            // condition until we trigger the event.
            //
            // Events are ordered by the ordering of signal()/is_pending().
            inner.was_cancelled.store(true, Ordering::Relaxed);
            inner.event.signal();
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("event trigger was dropped before being triggered")]
pub(crate) struct TriggerDropped;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TryWaitResult {
    Triggered,
    TriggerDropped,
    Pending,
}

#[derive(Clone)]
pub(crate) struct EventWaiter(Arc<PairInner>);

impl EventWaiter {
    pub(crate) fn wait_sync(&self) -> Result<(), TriggerDropped> {
        self.0.event.wait_sync();
        if self.0.was_cancelled.load(Ordering::Relaxed) {
            return Err(TriggerDropped);
        }
        Ok(())
    }

    pub(crate) fn try_wait(&self) -> TryWaitResult {
        if self.0.event.is_pending() {
            return TryWaitResult::Pending;
        }
        if self.0.was_cancelled.load(Ordering::Relaxed) {
            return TryWaitResult::TriggerDropped;
        }
        TryWaitResult::Triggered
    }

    pub(crate) async fn wait_async(&self) -> Result<(), TriggerDropped> {
        self.0.event.wait_async().await;
        if self.0.was_cancelled.load(Ordering::Relaxed) {
            return Err(TriggerDropped);
        }
        Ok(())
    }
}

pub(crate) fn new_event_pair() -> (EventTrigger, EventWaiter) {
    let inner = Arc::new(PairInner {
        event: Event::new(),
        was_cancelled: AtomicBool::new(false),
    });

    (EventTrigger(Some(inner.clone())), EventWaiter(inner))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_trigger() {
        let (trigger, waiter) = new_event_pair();
        assert_eq!(waiter.try_wait(), TryWaitResult::Pending);

        std::thread::spawn(move || {
            trigger.trigger();
        });

        assert_eq!(waiter.wait_sync(), Ok(()));
        assert_eq!(waiter.try_wait(), TryWaitResult::Triggered);
    }

    #[test]
    fn test_sync_trigger_dropped() {
        let (trigger, waiter) = new_event_pair();
        assert_eq!(waiter.try_wait(), TryWaitResult::Pending);

        std::thread::spawn(move || {
            drop(trigger);
        });

        assert_eq!(waiter.wait_sync(), Err(TriggerDropped));
        assert_eq!(waiter.try_wait(), TryWaitResult::TriggerDropped);
    }

    #[test]
    fn test_async_trigger() {
        let (trigger, waiter) = new_event_pair();

        let handle = std::thread::spawn(move || {
            futures_executor::block_on(async move { waiter.wait_async().await })
        });

        trigger.trigger();
        assert_eq!(handle.join().unwrap(), Ok(()));
    }

    #[test]
    fn test_async_trigger_dropped() {
        let (trigger, waiter) = new_event_pair();

        let handle = std::thread::spawn(move || {
            futures_executor::block_on(async move { waiter.wait_async().await })
        });

        drop(trigger);
        assert_eq!(handle.join().unwrap(), Err(TriggerDropped));
    }

    #[test]
    fn test_future_drop_cleans_up_waiter() {
        let event = Event::new();
        assert!(event.is_pending());

        {
            let mut fut = std::pin::pin!(event.wait_async());
            let waker = std::task::Waker::noop();
            let mut cx = std::task::Context::from_waker(&waker);

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
