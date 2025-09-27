use crate::fs::err_helpers::io_async_bail;

/// Provides an upper bound on the data that can be read from a reader. If the
/// data available from the reader exceeds the limit, it will return an
/// [`std::io::ErrorKind::UnexpectedEof`] error.
#[pin_project::pin_project]
pub struct LengthLimitedAsyncReader<R> {
    #[pin]
    inner: R,
    remaining: u64,
}

impl<R> LengthLimitedAsyncReader<R> {
    /// Creates a new `LengthLimitedAsyncReader` that wraps the given reader
    /// and limits the amount of data that can be read to `max_bytes`.
    ///
    /// # Panics
    ///
    /// Panics if `max_bytes` is zero.
    #[must_use]
    pub fn new(inner: R, max_bytes: u64) -> Self {
        assert!(max_bytes > 0, "max_bytes must be greater than zero");
        Self {
            inner,
            remaining: max_bytes,
        }
    }
}

impl<R> tokio::io::AsyncRead for LengthLimitedAsyncReader<R>
where
    R: tokio::io::AsyncRead + Unpin,
{
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let initial_remaining = buf.remaining() as u64;
        let proj_self = self.project();
        futures::ready!(proj_self.inner.poll_read(cx, buf))?;
        let read_bytes = initial_remaining - buf.remaining() as u64;
        *proj_self.remaining = proj_self.remaining.saturating_sub(read_bytes);
        if *proj_self.remaining == 0 {
            // The underlying reader has provided more data than we expected.
            io_async_bail!(UnexpectedEof, "Input longer than expected.");
        }

        std::task::Poll::Ready(Ok(()))
    }
}
