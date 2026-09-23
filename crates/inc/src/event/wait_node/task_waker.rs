use std::{
    num::NonZero,
    sync::{Arc, Mutex},
    task::{Context, Waker},
};

/// A reference to a shared input that is used to wake a single task.
#[derive(Clone)]
pub(super) struct TaskWaker(Arc<Mutex<Option<Waker>>>);

impl TaskWaker {
    pub(super) fn new() -> Self {
        Self(Arc::new(Mutex::new(None)))
    }

    pub(super) fn set_waker(&self, cx: &mut Context<'_>) {
        let mut guard = self.0.lock().unwrap();
        if let Some(curr_waker) = guard.as_mut()
            && cx.waker().will_wake(curr_waker)
        {
            // No need to update.
            return;
        }
        *guard = Some(cx.waker().clone());
    }

    pub(super) fn wake(&self) {
        if let Some(waker) = self.0.lock().unwrap().take() {
            waker.wake();
        }
    }

    pub(super) fn id(&self) -> NonZero<usize> {
        NonZero::new(Arc::as_ptr(&self.0).addr()).expect("Arc pointer should not be null")
    }
}
