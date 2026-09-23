mod oneshot;
mod semaphore;
pub(crate) mod wait_node;

use std::fmt::Debug;

pub(crate) use crate::event::oneshot::Event;
use crate::event::wait_node::EventToken;

pub(crate) struct EventTrigger(crate::event::wait_node::EventTrigger);

impl EventTrigger {
    pub(crate) fn trigger(&mut self) {
        let _ = self.0.trigger();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("event trigger was dropped before being triggered")]
pub(crate) struct TriggerDropped;

#[derive(Clone)]
pub(crate) struct EventWaiter(EventToken);

impl EventWaiter {
    /// Creates an `EventWaiter` that is already marked as ready, so anything that
    /// tries to wait on it will immediately return/continue.
    pub(crate) fn new_ready() -> EventWaiter {
        EventWaiter(EventToken::new_triggered())
    }

    /// Creates an `EventWaiter` that is already marked as cancelled, so anything that
    /// tries to wait on it will immediately return/continue with `TriggerDropped`.
    #[expect(dead_code)]
    pub(crate) fn new_cancelled() -> EventWaiter {
        EventWaiter(EventToken::new_dropped())
    }

    pub(crate) fn wait(&self) -> EventToken {
        self.0.clone()
    }
}

pub(crate) fn new_event_pair() -> (EventTrigger, EventWaiter) {
    let (token, trigger) = EventToken::new();

    let trigger = EventTrigger(trigger);
    let waiter = EventWaiter(token);

    (trigger, waiter)
}

#[cfg(test)]
mod tests {
    use crate::event::wait_node::DroppedError;

    use super::*;

    #[test]
    fn test_sync_trigger() {
        let (mut trigger, waiter) = new_event_pair();
        let mut token = waiter.wait();
        assert_eq!(token.try_wait(), None);

        std::thread::spawn(move || {
            trigger.trigger();
        });

        assert_eq!(token.wait_sync(), Ok(()));
        assert_eq!(token.try_wait(), Some(Ok(())));
    }

    #[test]
    fn test_sync_trigger_dropped() {
        let (trigger, waiter) = new_event_pair();
        let mut token = waiter.wait();
        assert_eq!(token.try_wait(), None);

        std::thread::spawn(move || {
            drop(trigger);
        });

        assert_eq!(token.wait_sync(), Err(DroppedError));
        assert_eq!(token.try_wait(), Some(Err(DroppedError)));
    }

    #[test]
    fn test_async_trigger() {
        let (mut trigger, waiter) = new_event_pair();

        let handle = std::thread::spawn(move || {
            futures_executor::block_on(async move { waiter.wait().await })
        });

        trigger.trigger();
        assert_eq!(handle.join().unwrap(), Ok(()));
    }

    #[test]
    fn test_async_trigger_dropped() {
        let (trigger, waiter) = new_event_pair();

        let handle = std::thread::spawn(move || {
            futures_executor::block_on(async move { waiter.wait().await })
        });

        drop(trigger);
        assert_eq!(handle.join().unwrap(), Err(DroppedError));
    }
}
