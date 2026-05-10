mod oneshot;

use std::task::{Poll, ready};

use futures_util::FutureExt;

use crate::imp::futures::sync::oneshot as imp_oneshot;

pub(crate) fn run_async<F>(fut: F) -> F::Output
where
    F: Future,
{
    assert!(
        tokio::runtime::Handle::try_current().is_err(),
        "Calling run_in_async while in an existing tokio task."
    );

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime");
    rt.block_on(fut)
}

pub(crate) struct Spawned<T> {
    join_handle: Option<tokio::task::JoinHandle<T>>,
}

impl<T> Spawned<T> {
    pub(crate) fn spawn<F>(fut: F) -> Self
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        Spawned {
            join_handle: Some(tokio::task::spawn(fut)),
        }
    }

    pub(crate) fn forget(mut self) {
        self.join_handle.take().unwrap();
    }
    pub(crate) fn abort(mut self) {
        self.join_handle.take().unwrap().abort();
    }
    pub(crate) fn abort_and_wait(mut self) -> impl Future<Output = Result<(), T>> {
        let join_handle = self.join_handle.take().unwrap();
        join_handle.abort();
        async move {
            match join_handle.await {
                // We aborted, but the task completed anyway.
                Ok(value) => Err(value),
                Err(join_error) => {
                    if join_error.is_cancelled() {
                        return Ok(());
                    }
                    match join_error.try_into_panic() {
                        Ok(panic) => {
                            // Propagate panic to this future.
                            std::panic::resume_unwind(panic)
                        }
                        Err(other) => panic!("Unknown join error: {other}"),
                    }
                }
            }
        }
    }
}

impl<T> Drop for Spawned<T> {
    fn drop(&mut self) {
        if let Some(handle) = self.join_handle.take() {
            handle.abort();
        }
    }
}

impl<T> Future for Spawned<T> {
    type Output = T;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let result = ready!(
            self.get_mut()
                .join_handle
                .as_mut()
                .expect("Cannot poll forgotten future")
                .poll_unpin(cx)
        );

        match result {
            Ok(value) => Poll::Ready(value),
            Err(join_error) => {
                // The only way we can be cancelled is if the Spawned handle
                // is consumed, but then this handle should no longer be
                // polled.
                assert!(!join_error.is_cancelled());
                match join_error.try_into_panic() {
                    Ok(panic) => {
                        // Propagate panic to this future.
                        std::panic::resume_unwind(panic)
                    }
                    Err(other) => panic!("Unknown join error: {other}"),
                }
            }
        }
    }
}

pub(crate) fn spawn_future<F>(fut: F) -> impl Future<Output = F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send,
{
    let join_handle = tokio::task::spawn(fut);

    async move {
        match join_handle.await {
            Ok(result) => result,
            Err(join_error) => {
                assert!(!join_error.is_cancelled(), "Handle is never cancelled.");
                match join_error.try_into_panic() {
                    Ok(panic) => {
                        // Propagate panic to this future.
                        std::panic::resume_unwind(panic)
                    }
                    Err(other) => panic!("Unknown join error: {other}"),
                }
            }
        }
    }
}

pub(crate) trait SenderExt<T> {
    fn send_and_forget(self, value: T);
}

impl<T> SenderExt<T> for imp_oneshot::Sender<T> {
    fn send_and_forget(self, value: T) {
        let err = self.send(value);
        drop(err);
    }
}
