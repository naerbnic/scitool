use std::task::Poll;

use crate::imp::futures::{prelude::*, sync::oneshot as imp_oneshot};

#[derive(Debug, thiserror::Error)]
#[error("Cancelled")]
pub(crate) struct CancelledError;

pub(crate) enum DroppedReceiverPolicy {
    Panic,
    Ignore,
}

impl DroppedReceiverPolicy {
    fn trigger_policy(&self) {
        match self {
            DroppedReceiverPolicy::Panic => panic!("Receiver dropped"),
            DroppedReceiverPolicy::Ignore => {}
        }
    }
}

pub(crate) struct Sender<T> {
    inner: imp_oneshot::Sender<T>,
    recv_policy: DroppedReceiverPolicy,
}

impl<T> Sender<T> {
    pub(crate) fn send(self, t: T) {
        let send_result = self.inner.send(t);

        match send_result {
            Ok(()) => {}
            Err(_) => self.recv_policy.trigger_policy(),
        }
    }

    /// Send the result of the passed-in future into this sender. If the
    /// receiver is dropped while this function is being called, it will
    /// be cancelled. The result of the send itself will follow the
    /// receive policy.
    pub(crate) async fn send_async<F>(mut self, f: F)
    where
        F: Future<Output = T>,
    {
        let f = std::pin::pin!(f);
        let send_result = tokio::select! {
            result = f => {
                Some(result)
            },
            () = self.inner.closed() => {
                None
            }
        };

        if let Some(result) = send_result {
            self.send(result);
        } else {
            self.recv_policy.trigger_policy();
        }
    }

    pub(crate) async fn try_send_async<F, E>(mut self, f: F) -> Result<(), E>
    where
        F: Future<Output = Result<T, E>>,
    {
        let f = std::pin::pin!(f);
        let send_result = tokio::select! {
            result = f => {
                Some(result)
            },
            () = self.inner.closed() => {
                None
            }
        };

        match send_result {
            Some(Ok(value)) => self.send(value),
            Some(Err(e)) => return Err(e),
            None => {
                self.recv_policy.trigger_policy();
            }
        }

        Ok(())
    }
}

pub(crate) struct Receiver<T> {
    inner: imp_oneshot::Receiver<T>,
}

impl<T> Receiver<T> {
    pub(crate) async fn recv_or_default<DefaultFut>(self, default_fut: DefaultFut) -> T
    where
        DefaultFut: Future<Output = T>,
    {
        if let Ok(r) = self.inner.await {
            r
        } else {
            default_fut.await
        }
    }
    pub(crate) async fn recv_or_else<ElseFut>(self, else_fut: ElseFut) -> Result<T, ElseFut::Output>
    where
        ElseFut: Future,
    {
        self.inner.or_else(async move |_| Err(else_fut.await)).await
    }
}

impl<T> Future for Receiver<T> {
    type Output = Result<T, CancelledError>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        Poll::Ready(
            std::task::ready!(self.get_mut().inner.poll_unpin(cx)).map_err(|_| CancelledError),
        )
    }
}

// Terrible name. Provide a nicer one.
pub(crate) struct PanicReceiver<T>(imp_oneshot::Receiver<T>);

impl<T> Future for PanicReceiver<T> {
    type Output = T;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        Poll::Ready(std::task::ready!(self.get_mut().0.poll_unpin(cx)).expect("Sender dropped."))
    }
}

pub(crate) fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let (imp_tx, imp_rx) = imp_oneshot::channel();
    (
        Sender {
            inner: imp_tx,
            recv_policy: DroppedReceiverPolicy::Ignore,
        },
        Receiver { inner: imp_rx },
    )
}

pub(crate) fn panic_channel<T>() -> (Sender<T>, PanicReceiver<T>) {
    let (imp_tx, imp_rx) = imp_oneshot::channel();
    (
        Sender {
            inner: imp_tx,
            recv_policy: DroppedReceiverPolicy::Ignore,
        },
        PanicReceiver(imp_rx),
    )
}
