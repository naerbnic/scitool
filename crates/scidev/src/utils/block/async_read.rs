use std::{
    io,
    sync::{Arc, Mutex},
    task::Poll,
};

use futures::FutureExt;
use tokio::{io::AsyncRead, task::JoinHandle};

struct Inner<R> {
    reader: R,
    buffer: Box<[u8]>,
    read_amount: usize,
}

/// An `AsyncRead` object that wraps a [`std::io::Read`] synchronous reader.
///
/// This operates by running any polled read operation in a `spawn_blocking()`
/// closure.
pub(super) struct AsyncReadWrapper<R> {
    inner: Arc<Mutex<Inner<R>>>,
    pending: Option<JoinHandle<io::Result<()>>>,
}

impl<R> AsyncReadWrapper<R>
where
    R: io::Read + Send + 'static,
{
    pub(super) fn new(reader: R) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                reader,
                buffer: vec![0u8; 8 * 1024].into_boxed_slice(),
                read_amount: 0,
            })),
            pending: None,
        }
    }
}

impl<R> AsyncRead for AsyncReadWrapper<R>
where
    R: io::Read + Send + 'static,
{
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        let this = self.get_mut();
        let pending = this.pending.get_or_insert_with(|| {
            // We have a blocking read that is not pending. Create a blocking
            // spawn to handle the operation.
            let inner = this.inner.clone();
            tokio::task::spawn_blocking(move || {
                let mut inner_guard = inner.try_lock().unwrap();
                let inner = &mut *inner_guard;
                inner.read_amount = inner.reader.read(&mut inner.buffer)?;
                Ok::<_, io::Error>(())
            })
        });
        // Poll the pending operation. This will register any wakers for us.
        std::task::ready!(pending.poll_unpin(cx))??;
        this.pending = None;

        // As we have a mut reference to the struct, we should be the only ones
        // with a remaining lock on the inner object, so we can take it without
        // blocking.
        let mut inner_guard = this.inner.try_lock().unwrap();
        buf.put_slice(&inner_guard.buffer[..inner_guard.read_amount]);
        inner_guard.read_amount = 0;
        Poll::Ready(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt as _;

    #[tokio::test]
    async fn test_async_read_wrapper() -> io::Result<()> {
        let data = b"hello world";
        let mut reader = AsyncReadWrapper::new(std::io::Cursor::new(data));
        let mut buf = [0u8; 11];
        reader.read_exact(&mut buf).await?;
        assert_eq!(data, &buf);
        Ok(())
    }
}
