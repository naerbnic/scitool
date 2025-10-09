use std::{
    pin::Pin,
    sync::{Arc, Mutex as SyncMutex},
};

use tokio::sync::Notify;

#[derive(Debug)]
struct DropInner {
    notify: Notify,
    closed: SyncMutex<bool>,
}

#[derive(Debug)]
pub(crate) struct DropDetector {
    inner: Arc<DropInner>,
    disconnected: bool,
}

impl DropDetector {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(DropInner {
                notify: Notify::new(),
                closed: SyncMutex::new(false),
            }),
            disconnected: false,
        }
    }

    pub(crate) fn get_handle(&self) -> DropHandle {
        DropHandle {
            inner: self.inner.clone(),
        }
    }

    /// Explicitly disconnect the detector without dropping it.
    /// This prevents any `DropHandle` from being notified.
    pub(crate) fn disconnect(&mut self) {
        self.disconnected = true;
    }
}

impl Drop for DropDetector {
    fn drop(&mut self) {
        if !self.disconnected {
            let mut closed = self.inner.closed.lock().unwrap();
            *closed = true;
            self.inner.notify.notify_waiters();
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DropHandle {
    inner: Arc<DropInner>,
}

impl DropHandle {
    pub(crate) fn is_dropped(&self) -> bool {
        self.inner.closed.lock().unwrap().to_owned()
    }

    async fn dropped(self) {
        // Take a notification future first, as it enqueues the waiter
        // before we check the drop. Otherwise there may be a race where
        // the drop happens between the check and the wait, which causes the
        // notify_waiter() call to be missed.
        let notified = self.inner.notify.notified();
        if self.is_dropped() {
            return;
        }
        notified.await;
    }
}

impl IntoFuture for DropHandle {
    type Output = ();

    type IntoFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.dropped())
    }
}

pub(crate) struct Token {
    handle: DropHandle,
}

impl Token {
    pub(crate) fn is_cancelled(&self) -> bool {
        self.handle.is_dropped()
    }
}

impl IntoFuture for Token {
    type Output = ();

    type IntoFuture = <DropHandle as IntoFuture>::IntoFuture;

    fn into_future(self) -> Self::IntoFuture {
        self.handle.into_future()
    }
}

pub(crate) struct Canceller {
    drop_detector: Option<DropDetector>,
}

impl Canceller {
    pub(crate) fn new() -> (Self, Token) {
        let detector = DropDetector::new();
        let token = Token {
            handle: detector.get_handle(),
        };
        (
            Self {
                drop_detector: Some(detector),
            },
            token,
        )
    }

    pub(crate) fn cancel(&mut self) {
        drop(self.drop_detector.take());
    }
}

impl Drop for Canceller {
    fn drop(&mut self) {
        // Dropping a canceller must not trigger cancellation. If we haven't
        // already been cancelled, disconnect the detector.
        if let Some(mut detector) = self.drop_detector.take() {
            detector.disconnect();
        }
    }
}
