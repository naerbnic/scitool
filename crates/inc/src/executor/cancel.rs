//! Implement types to handle cancellation within an executor.
//!
//! Async runtimes allow futures to be cancelled just by dropping (assuming that
//! they're cancel safe, by some definition). This isn't the case for synchronous
//! code, which needs to check for a current cancellation state. Our
//! synchronization primitves can use this tool in a synchronous context, yielding
//! a propagatable cancellation error.

use std::{
    sync::{Arc, Mutex},
    task::{Poll, ready},
};

use pin_project::pin_project;

#[derive(Debug, thiserror::Error)]
#[error("Operation cancelled")]
pub(crate) struct CancelledError;

#[derive(Debug, thiserror::Error)]
pub(crate) enum MaybeCancelledError<T> {
    Cancelled,
    Error(T),
}

pub(crate) type CancelResult<T, E> = Result<T, MaybeCancelledError<E>>;

/// A type that can express cancellation (or lack thereof), and return
/// a value if desired.
pub(crate) trait IntoCancellableReturn {
    type Output;

    fn into_cancellable_return(self) -> Result<Self::Output, CancelledError>;
}

impl<T, E> IntoCancellableReturn for CancelResult<T, E> {
    type Output = Result<T, E>;

    fn into_cancellable_return(self) -> Result<Self::Output, CancelledError> {
        match self {
            Ok(val) => Ok(Ok(val)),
            Err(MaybeCancelledError::Cancelled) => Err(CancelledError),
            Err(MaybeCancelledError::Error(e)) => Ok(Err(e)),
        }
    }
}

impl<T> IntoCancellableReturn for Result<T, CancelledError> {
    type Output = T;

    fn into_cancellable_return(self) -> Result<Self::Output, CancelledError> {
        match self {
            Ok(val) => Ok(val),
            Err(e) => Err(e),
        }
    }
}

impl IntoCancellableReturn for () {
    type Output = ();

    fn into_cancellable_return(self) -> Result<Self::Output, CancelledError> {
        Ok(self)
    }
}

struct OnCancelState {
    waiter: Option<std::task::Waker>,
    is_cancelled: bool,
}

pub(crate) struct CancelHandle {
    state: Arc<Mutex<OnCancelState>>,
}

impl CancelHandle {
    pub(crate) fn cancel(self) {
        let mut guard = self.state.lock().unwrap();
        guard.is_cancelled = true;
        if let Some(waker) = guard.waiter.take() {
            waker.wake();
        }
    }
}

#[pin_project]
pub(crate) struct CancellableFuture<F> {
    state: Arc<Mutex<OnCancelState>>,
    #[pin]
    child: F,
}

impl<F> CancellableFuture<F>
where
    F: Future,
{
    pub(crate) fn new(f: F) -> (Self, CancelHandle) {
        let state = Arc::new(Mutex::new(OnCancelState {
            waiter: None,
            is_cancelled: false,
        }));
        (
            Self {
                state: state.clone(),
                child: f,
            },
            CancelHandle { state },
        )
    }
}

impl<F> Future for CancellableFuture<F>
where
    F: Future,
{
    type Output = Result<F::Output, CancelledError>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.project();
        {
            let mut guard = this.state.lock().unwrap();

            if guard.is_cancelled {
                return std::task::Poll::Ready(Err(CancelledError));
            }

            guard.waiter = Some(cx.waker().clone());
        }
        Poll::Ready(Ok(ready!(this.child.poll(cx))))
    }
}
