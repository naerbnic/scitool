use std::collections::VecDeque;

use crate::{
    LockType,
    waiter::{Waiter, Waker},
};

struct WaitEntry<T> {
    lock_type: LockType,
    waker: Waker<T>,
}

pub(super) struct WaitQueue<T> {
    entries: VecDeque<WaitEntry<T>>,
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

    pub(super) fn push(&mut self, lock_type: LockType) -> Waiter<T> {
        let (waiter, waker) = Waiter::new();
        // Exclusive locks always need a new wait group.
        self.entries.push_back(WaitEntry { lock_type, waker });
        waiter
    }

    pub(super) fn next_mut(&mut self) -> Option<QueueFrontGuard<'_, T>> {
        let front = self.entries.front()?;

        Some(QueueFrontGuard {
            lock_type: front.lock_type,
            queue: &mut self.entries,
        })
    }
}

pub(super) struct QueueFrontGuard<'a, T> {
    queue: &'a mut VecDeque<WaitEntry<T>>,
    lock_type: LockType,
}

impl<T> QueueFrontGuard<'_, T> {
    pub(super) fn lock_type(&self) -> LockType {
        self.lock_type
    }

    pub(super) fn take_waiter(self) -> Waker<T> {
        self.queue.pop_front().unwrap().waker
    }
}
