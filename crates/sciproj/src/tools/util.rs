use std::{
    io,
    pin::Pin,
    sync::{Arc, atomic::Ordering},
    task::{Context, Poll},
};

use futures_util::future::{Either, Ready, ready};
use pin_project::pin_project;
use tokio::{
    io::{AsyncRead, ReadBuf},
    sync::{Notify, futures::Notified},
};

struct NotifyOnce {
    notify: Notify,
    done: std::sync::atomic::AtomicBool,
}

#[pin_project]
struct NotifyOnceNotified<'a> {
    #[pin]
    notified: Either<Notified<'a>, Ready<()>>,
}

impl Future for NotifyOnceNotified<'_> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().notified.poll(cx)
    }
}

impl NotifyOnce {
    #[must_use]
    fn new() -> Self {
        Self {
            notify: Notify::new(),
            done: std::sync::atomic::AtomicBool::new(false),
        }
    }

    fn notify(&self) {
        // Our order has to be a little careful. We must set done before
        // notifying, otherwise we could have a race where a waiter checks
        // done, then runs `self.notify.notified()`, but it is not notified
        // again, as the notifier has already completed.
        if self
            .done
            .compare_exchange(false, true, Ordering::Release, Ordering::Relaxed)
            .is_err()
        {
            // Done is already marked as such, we can short circuit.
            return;
        }

        // Notify the waiters. This should cause all waiters to be woken, or
        // future calls to notified should return immediately completing
        // futures.
        self.notify.notify_waiters();
    }

    fn notified(&self) -> NotifyOnceNotified<'_> {
        // We do this carefully. These two sequences have to be interleaved,
        // and succeed in all valid circumstances:

        /*
        Notifier:

        - N1: Set Done = true
        - N2: Notify all waiters

        Waiter:

        - W1: Obtain Notified instance, beginning waiting
        - W2: Check done: if true, drop notified and continue without waiting.
        - W3: Wait on Notified instance.

        Properties:
        - If W2 comes after N1, then the Waiter will continue without waiting.
        - If N2 comes after W1, then the Notified instance will immediately be
          notified, even without seeing done = true.
        - N1 must come before N2 (using atomic ordering)
        - W1 must come before W2 (using atomic ordering)

        The only way for this to fail is if W2 ~> N1, and N2 ~> W1, but if
        those are both true, then either N2 ~> N1, or W2 ~> W1, both of which
        are violations of our properties.
        */
        let possible_notified = self.notify.notified();
        // This uses Release ordering, to ensure that possible_notified is created before
        // the read of done.
        let done = self.done.load(Ordering::Acquire);
        NotifyOnceNotified {
            notified: if done {
                Either::Right(ready(()))
            } else {
                Either::Left(possible_notified)
            },
        }
    }
}

// Question: Is there a genuine safe type that allows you to have direct projections through an Arc type?
// i.e., a value that is contained within another value that maintains an inner read lifetime?
// It would need to use a phantom lifetime in order to ensure that the returned value is still
// tied to the lifetime of the Arc.

struct CancelInner {
    /// Notified when
    child_drop_notifier: NotifyOnce,
    child_count: std::sync::atomic::AtomicUsize,
    complete_notifier: NotifyOnce,
}

/// An internal token that has a Drop that handles correct handling of the
/// future drop.
struct CancelFutureDropToken {
    inner: Arc<CancelInner>,
}

impl Drop for CancelFutureDropToken {
    fn drop(&mut self) {
        self.inner.child_drop_notifier.notify();
    }
}

#[pin_project]
pub(super) struct CancelFuture<F> {
    #[pin]
    inner_fut: F,
    drop_token: CancelFutureDropToken,
}

impl<F> Future for CancelFuture<F>
where
    F: Future,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().inner_fut.poll(cx)
    }
}

pub(super) struct CancelHandle {
    inner: Arc<CancelInner>,
}

impl CancelHandle {
    pub(super) async fn children_dropped(&self) {
        self.inner.complete_notifier.notified().await;
    }
}

#[pin_project]
pub(super) struct ChildrenDropped<'a> {
    #[pin]
    inner: NotifyOnceNotified<'a>,
}

impl Future for ChildrenDropped<'_> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().inner.poll(cx)
    }
}

/// A token holding "interest" in a pending async. If all cancel tokens for
/// a cancellable future are dropped, the future itself will be dropped.
///
/// If the future this is pending is cancelled/dropped by other means, the
/// `Self::future_dropped()` future will be notified.
pub(super) struct CancelToken {
    inner: Arc<CancelInner>,
}

impl CancelToken {
    pub(super) fn new<F, R>(fut: F) -> (Self, CancelFuture<R>)
    where
        F: FnOnce(CancelHandle) -> R,
        R: Future,
    {
        cancellable_future(fut)
    }
}

impl Drop for CancelToken {
    fn drop(&mut self) {
        if self.inner.child_count.fetch_sub(1, Ordering::Acquire) == 1 {
            self.inner.child_drop_notifier.notify();
        }
    }
}

impl Clone for CancelToken {
    fn clone(&self) -> Self {
        self.inner.child_count.fetch_add(1, Ordering::Relaxed);
        Self {
            inner: self.inner.clone(),
        }
    }
}

#[pin_project]
pub(super) struct FutureDropped<'a> {
    #[pin]
    inner: NotifyOnceNotified<'a>,
}

impl Future for FutureDropped<'_> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().inner.poll(cx)
    }
}

fn cancellable_future<F, R>(fut: F) -> (CancelToken, CancelFuture<R>)
where
    F: FnOnce(CancelHandle) -> R,
    R: Future,
{
    let inner = Arc::new(CancelInner {
        child_drop_notifier: NotifyOnce::new(),
        child_count: std::sync::atomic::AtomicUsize::new(1),
        complete_notifier: NotifyOnce::new(),
    });
    let token = CancelToken {
        inner: inner.clone(),
    };
    let inner_fut = {
        let handle = CancelHandle {
            inner: inner.clone(),
        };
        fut(handle)
    };
    let future = CancelFuture {
        inner_fut,
        drop_token: CancelFutureDropToken { inner },
    };
    (token, future)
}

#[pin_project]
pub(super) struct ProcessAsyncReader<R> {
    #[pin]
    inner_reader: R,
    cancel_token: CancelToken,
}

impl<R: AsyncRead> ProcessAsyncReader<R> {
    pub(super) fn new(reader: R, cancel: CancelToken) -> Self {
        Self {
            inner_reader: reader,
            cancel_token: cancel,
        }
    }
}

impl<R: AsyncRead> AsyncRead for ProcessAsyncReader<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        self.project().inner_reader.poll_read(cx, buf)
    }
}
