use std::collections::VecDeque;

use crate::{
    LockType,
    waiter::{Waiter, Waker},
};

struct WaitGroup<T> {
    lock_type: LockType,
    wakers: Vec<Waker<T>>,
}

pub(super) struct WaitQueue<T> {
    entries: VecDeque<WaitGroup<T>>,
}

impl<T> WaitQueue<T> {
    pub(super) fn new() -> Self {
        Self {
            entries: VecDeque::new(),
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(super) fn push_empty(&mut self, lock_type: LockType) {
        self.entries.push_back(WaitGroup {
            lock_type,
            wakers: Vec::new(),
        });
    }

    pub(super) fn push(&mut self, lock_type: LockType) -> Waiter<T> {
        let (waiter, waker) = Waiter::new();
        if let LockType::Exclusive = lock_type {
            // Exclusive locks always need a new wait group.
            self.entries.push_back(WaitGroup {
                lock_type,
                wakers: vec![waker],
            });
            return waiter;
        }

        if let Some(queue_back) = self.entries.back_mut()
            && queue_back.lock_type == lock_type
        {
            queue_back.wakers.push(waker);
            return waiter;
        }
        self.entries.push_back(WaitGroup {
            lock_type,
            wakers: vec![waker],
        });
        waiter
    }

    pub(super) fn front_mut(&mut self) -> Option<QueueFrontGuard<'_, T>> {
        let front = self.entries.front()?;

        Some(QueueFrontGuard {
            lock_type: front.lock_type,
            queue: &mut self.entries,
        })
    }
}

pub(super) struct QueueFrontGuard<'a, T> {
    queue: &'a mut VecDeque<WaitGroup<T>>,
    lock_type: LockType,
}

impl<T> QueueFrontGuard<'_, T> {
    pub(super) fn lock_type(&self) -> LockType {
        self.lock_type
    }
    pub(super) fn take_first_waiter(&mut self) -> Option<Waker<T>> {
        let front = self.queue.front_mut().unwrap();
        front.wakers.pop()
    }
    pub(super) fn take_all_waiters(self) -> Vec<Waker<T>> {
        let front = self.queue.pop_front().unwrap();
        front.wakers
    }

    pub(super) fn drop_empty(self) {
        assert!(self.queue.front().unwrap().wakers.is_empty());
        self.queue.pop_front();
    }
}
