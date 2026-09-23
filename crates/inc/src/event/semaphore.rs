use std::{fmt::Debug, sync::Mutex};

use crate::event::wait_node::{EventToken, EventTrigger, WaitSet};

/// An object representing a state that can be activated at a specific time.
pub(crate) struct Semaphore(Mutex<SemState>);

struct SemState {
    waiters: WaitSet,
    pending_tickets: usize,
}

impl Debug for Semaphore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Semaphore").finish()
    }
}

impl Semaphore {
    pub(crate) fn new(init: usize) -> Self {
        Self(Mutex::new(SemState {
            waiters: WaitSet::new(),
            pending_tickets: init,
        }))
    }

    pub(crate) fn wait(&self) -> EventToken {
        let (token, trigger) = EventToken::new();
        self.add_waiter(trigger);
        token
    }

    fn add_waiter(&self, waiter: EventTrigger) {
        let mut guard = self.0.lock().unwrap();
        if let Some(new_pending_tickets) = guard.pending_tickets.checked_sub(1) {
            if waiter.trigger().is_ok() {
                guard.pending_tickets = new_pending_tickets;
            }
            return;
        }

        guard.waiters.add_trigger(&waiter);
    }

    /// Cause the semaphore to be signaled.
    pub(crate) fn signal(&self) {
        let mut guard = self.0.lock().unwrap();
        if !guard.waiters.trigger_next() {
            guard.pending_tickets += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use super::*;

    #[test]
    fn test_sync_available_tickets() {
        let sem = Semaphore::new(2);
        sem.wait().wait_sync();
        sem.wait().wait_sync();
        let guard = sem.0.lock().unwrap();
        assert_eq!(guard.pending_tickets, 0);
    }

    #[test]
    fn test_sync_blocking_and_signal() {
        let sem = Arc::new(Semaphore::new(0));
        let sem_clone = Arc::clone(&sem);
        let flag = Arc::new(AtomicBool::new(false));
        let flag_clone = Arc::clone(&flag);

        let handle = std::thread::spawn(move || {
            sem_clone.wait().wait_sync();
            flag_clone.store(true, Ordering::SeqCst);
        });

        std::thread::sleep(Duration::from_millis(20));
        assert!(!flag.load(Ordering::SeqCst));

        sem.signal();
        handle.join().unwrap();
        assert!(flag.load(Ordering::SeqCst));
    }

    #[test]
    fn test_async_available_tickets() {
        let sem = Semaphore::new(1);
        futures_executor::block_on(async {
            sem.wait().await;
        });
        let guard = sem.0.lock().unwrap();
        assert_eq!(guard.pending_tickets, 0);
    }

    #[test]
    fn test_async_blocking_and_signal() {
        let sem = Arc::new(Semaphore::new(0));
        let sem_clone = Arc::clone(&sem);

        let handle = std::thread::spawn(move || {
            futures_executor::block_on(async move {
                sem_clone.wait().await;
            });
        });

        std::thread::sleep(Duration::from_millis(20));
        sem.signal();
        handle.join().unwrap();
    }

    #[test]
    fn test_fifo_wakeup_order() {
        let sem = Arc::new(Semaphore::new(0));
        let order = Arc::new(Mutex::new(Vec::new()));

        let mut handles = Vec::new();
        for i in 0..3 {
            let sem_clone = Arc::clone(&sem);
            let order_clone = Arc::clone(&order);
            let handle = std::thread::spawn(move || {
                sem_clone.wait().wait_sync();
                order_clone.lock().unwrap().push(i);
            });
            std::thread::sleep(Duration::from_millis(15));
            handles.push(handle);
        }

        for _ in 0..3 {
            sem.signal();
            std::thread::sleep(Duration::from_millis(15));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let recorded_order = order.lock().unwrap().clone();
        assert_eq!(recorded_order, vec![0, 1, 2]);
    }

    #[test]
    fn test_mutual_exclusion() {
        let sem = Arc::new(Semaphore::new(1));
        let counter = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();

        for _ in 0..8 {
            let sem = Arc::clone(&sem);
            let counter = Arc::clone(&counter);
            handles.push(std::thread::spawn(move || {
                for _ in 0..100 {
                    sem.wait().wait_sync();
                    let val = counter.fetch_add(1, Ordering::SeqCst);
                    std::thread::yield_now();
                    assert_eq!(counter.load(Ordering::SeqCst), val + 1);
                    sem.signal();
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(counter.load(Ordering::SeqCst), 800);
    }

    #[test]
    fn test_async_drop_cancellation_with_stale_queue() {
        let sem = Arc::new(Semaphore::new(0));

        let waker = std::task::Waker::noop();
        let mut cx = std::task::Context::from_waker(waker);

        // Register and then drop fut1 so it leaves a stale key in queue
        {
            let mut fut1 = std::pin::pin!(sem.wait());
            assert_eq!(fut1.as_mut().poll(&mut cx), std::task::Poll::Pending);
        }

        let mut fut2 = std::pin::pin!(sem.wait());
        assert_eq!(fut2.as_mut().poll(&mut cx), std::task::Poll::Pending);

        // signal() should skip the stale fut1 key in the queue and wake fut2
        sem.signal();
        assert_eq!(fut2.as_mut().poll(&mut cx), std::task::Poll::Ready(Ok(())));
    }
}
