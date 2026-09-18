use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    thread::JoinHandle,
    time::{Duration, Instant},
};

use crate::{
    clock::Clock,
    event::{Event, EventTrigger},
};

pub(crate) struct Scheduler {
    handle: Option<JoinHandle<()>>,
    inner: Arc<Inner>,
}

impl Scheduler {
    pub(crate) fn new<C>(clock: C) -> Self
    where
        C: Clock + Send + Sync + 'static,
    {
        let inner = Arc::new(Inner::new(clock));
        let handle = std::thread::spawn({
            let inner = inner.clone();
            move || {
                loop {
                    if inner.is_cancelled() {
                        return;
                    }

                    match inner.signal_to_now() {
                        Some(timeout_duration) => {
                            std::thread::park_timeout(timeout_duration);
                        }
                        None => std::thread::park(),
                    }
                }
            }
        });
        Self {
            handle: Some(handle),
            inner,
        }
    }

    fn trigger_at(&self, deadline: Instant, mut event: EventTrigger) {
        if deadline < self.inner.clock.now() {
            event.trigger();
            return;
        }
        if self.inner.add_event(deadline, event)
            && let Some(handle) = self.handle.as_ref()
        {
            handle.thread().unpark();
        }
    }
}

impl Drop for Scheduler {
    fn drop(&mut self) {
        self.inner.cancel_event.signal();
        if let Some(handle) = self.handle.take() {
            handle.thread().unpark();
            handle.join().unwrap();
        }
    }
}

struct Inner {
    clock: Box<dyn Clock + Send + Sync>,
    queue: Mutex<BTreeMap<Instant, Vec<EventTrigger>>>,
    cancel_event: Event,
}

impl Inner {
    fn new<C>(clock: C) -> Self
    where
        C: Clock + Send + Sync + 'static,
    {
        Self {
            clock: Box::new(clock),
            queue: Mutex::default(),
            cancel_event: Event::new(),
        }
    }

    fn signal_to_now(&self) -> Option<Duration> {
        let now = self.clock.now();
        let mut guard = self.queue.lock().unwrap();
        loop {
            let entry = guard.first_entry()?;
            if *entry.key() > now {
                break Some(*entry.key() - now);
            }
            for mut trigger in entry.remove() {
                trigger.trigger();
            }
        }
    }

    // Return true if this needs to interrupt the scheduler thread.
    fn add_event(&self, deadline: Instant, event: EventTrigger) -> bool {
        let mut guard = self.queue.lock().unwrap();
        let mut needs_reschedule = false;
        if let Some((next_instant, _)) = guard.first_key_value()
            && next_instant > &deadline
        {
            needs_reschedule = true;
        }
        guard.entry(deadline).or_default().push(event);
        needs_reschedule
    }

    fn is_cancelled(&self) -> bool {
        !self.cancel_event.is_pending()
    }
}
