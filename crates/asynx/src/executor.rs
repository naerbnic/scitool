//! Tools for minimal async execution. These do not imply a runtime, and
//! should work within any runtime if the constraints are met.

use std::{
    pin::pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll},
};

#[derive(Debug)]
struct ThreadWaker {
    thread: std::thread::Thread,
    woken: AtomicBool,
}

impl ThreadWaker {
    fn wake_thread(&self) {
        let already_woken = self.woken.swap(true, Ordering::Release);
        if !already_woken {
            self.thread.unpark();
        }
    }
}

impl std::task::Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.wake_thread();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.wake_thread();
    }
}

pub fn block_on_poller<F, R>(mut poll_fn: F) -> R
where
    F: FnMut(&mut Context<'_>) -> Poll<R>,
{
    crate::flavor::assert_in_sync();
    let thread_waker = Arc::new(ThreadWaker {
        thread: std::thread::current(),
        woken: AtomicBool::new(false),
    });
    let waker = thread_waker.clone().into();
    let mut cx = Context::from_waker(&waker);
    loop {
        if let Poll::Ready(ret) = crate::flavor::run_wth_async_scope(|| poll_fn(&mut cx)) {
            break ret;
        }

        while !thread_waker.woken.swap(false, Ordering::Acquire) {
            std::thread::park();
        }
    }
}

/// A minimal implementation of `block_on`, that blocks the current thread
/// entirely and only polls the provided future until it completes. This will
/// correctly respond to the future protocol, but will only run this single
/// future.
///
/// Any subtasks must be scheduled with a separate runtime, or the context
/// provided after the call.
pub fn block_on<F>(fut: F) -> F::Output
where
    F: Future,
{
    let mut fut = pin!(fut);
    block_on_poller(|cx| fut.as_mut().poll(cx))
}
