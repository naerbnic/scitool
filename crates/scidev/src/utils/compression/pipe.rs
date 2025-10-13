//! Leverage the state-machine properties of async to provide a way
//! to write both push and pull style data processing as an async pipe,
//! callable entirely in synchronous code.

use std::{io, task::Poll};

use futures::{
    FutureExt,
    io::{AsyncRead, AsyncWrite},
};
pub(super) trait DataProcessor {
    fn process<R, W>(self, reader: R, writer: W) -> impl Future<Output = Result<(), io::Error>>
    where
        R: AsyncRead + Unpin,
        W: AsyncWrite + Unpin;

    #[expect(dead_code, reason = "Will be used in lazy block")]
    fn process_sync<R, W>(self, reader: R, writer: W) -> Result<(), io::Error>
    where
        Self: Sized,
        R: io::Read + Unpin,
        W: io::Write + Unpin,
    {
        let waker = std::task::Waker::noop();
        let mut cx = std::task::Context::from_waker(waker);
        let mut fut =
            std::pin::pin!(self.process(SyncAdapter::new(reader), SyncAdapter::new(writer)));
        match fut.poll_unpin(&mut cx) {
            Poll::Ready(r) => r,
            Poll::Pending => Err(io::Error::other(
                "DataProcessor cannot complete synchronously",
            )),
        }
    }

    fn pull<'a, R>(self, reader: R, buffer_capacity: usize) -> impl io::Read + 'a
    where
        Self: Sized + 'static,
        R: io::Read + Unpin + 'static,
    {
        inv_writer::pull_mode(self, reader, buffer_capacity)
    }

    #[expect(dead_code, reason = "Will be used in lazy block")]
    fn push<'a, W>(self, writer: W, buffer_capacity: usize) -> impl io::Write + 'a
    where
        Self: Sized + 'a,
        W: io::Write + Unpin + 'a,
    {
        inv_reader::push_mode(self, writer, buffer_capacity)
    }
}

/// A wrapper that runs any async reader/writer as a synchronous reader/writer.
///
/// This would be a terrible idea if this were run on a general-purpose
/// executor, but this will only be used in this context, to allow for
/// a data processing function to be written once for both push and pull
/// styles.
struct SyncAdapter<T> {
    inner: T,
}

impl<T> SyncAdapter<T> {
    fn new(inner: T) -> Self {
        Self { inner }
    }
}

impl<R> AsyncRead for SyncAdapter<R>
where
    R: io::Read + Unpin,
{
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        Poll::Ready(self.get_mut().inner.read(buf))
    }
}

impl<W> AsyncWrite for SyncAdapter<W>
where
    W: io::Write + Unpin,
{
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Poll::Ready(self.get_mut().inner.write(buf))
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Poll::Ready(self.get_mut().inner.flush())
    }

    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        // No-op for synchronous writers.
        Poll::Ready(Ok(()))
    }
}

mod inv_reader {
    use std::{
        cell::RefCell,
        collections::VecDeque,
        io,
        pin::Pin,
        rc::Rc,
        task::{Context, Poll, Waker},
    };

    use bytes::Buf as _;
    use futures::AsyncRead;

    struct Inner {
        buffer: VecDeque<u8>,
        closed: bool,
        bytes_requested: usize,
    }

    struct InvertedReader {
        inner: Rc<RefCell<Inner>>,
    }

    impl AsyncRead for InvertedReader {
        fn poll_read(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<io::Result<usize>> {
            let mut inner = self.inner.borrow_mut();
            let to_read = std::cmp::min(buf.len(), inner.buffer.len());
            // If the buffer is empty, either we're done, or we need to wait
            // for more data.
            if to_read == 0 {
                return if inner.closed {
                    Poll::Ready(Ok(0))
                } else {
                    // If this were a normal async reader, we'd register a waker
                    // here, but since this is only used in a synchronous context,
                    // we can just return Pending and let the caller try again later.
                    inner.bytes_requested = std::cmp::min(buf.len(), inner.buffer.capacity());
                    Poll::Pending
                };
            }
            inner.buffer.copy_to_slice(&mut buf[..to_read]);
            Poll::Ready(Ok(to_read))
        }
    }

    pub(super) struct Writer<'a> {
        reader_state: Rc<RefCell<Inner>>,
        polled_future: Option<Pin<Box<dyn futures::Future<Output = io::Result<()>> + 'a>>>,
    }

    impl Writer<'_> {
        fn poll(&mut self) -> io::Result<()> {
            let mut cx = Context::from_waker(Waker::noop());
            let Some(fut) = &mut self.polled_future else {
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "Writer polled after completion",
                ));
            };
            match fut.as_mut().poll(&mut cx) {
                Poll::Ready(Ok(())) => {
                    // The future has completed, which could be fine. If we
                    // still have data in the buffer, that would be the
                    // approximate equivalent error of a BrokenPipe.
                    let mut inner = self.reader_state.borrow_mut();
                    inner.closed = true;
                    self.polled_future = None;
                    if !inner.buffer.is_empty() {
                        return Err(io::Error::new(
                            io::ErrorKind::BrokenPipe,
                            "Writer finished but reader has not consumed all data",
                        ));
                    }
                }
                Poll::Ready(Err(e)) => return Err(e),
                Poll::Pending => {}
            }
            Ok(())
        }
    }

    impl io::Write for Writer<'_> {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let mut inner = self.reader_state.borrow_mut();
            if inner.closed {
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "Writer attempted to write to closed reader",
                ));
            }
            let remaining_capacity = inner.buffer.capacity() - inner.buffer.len();
            let to_write = std::cmp::min(buf.len(), remaining_capacity);
            inner.buffer.extend(&buf[..to_write]);
            if inner.buffer.len() >= inner.bytes_requested {
                // If we've satisfied the requested bytes, clear the request
                // so that the reader can proceed.
                inner.bytes_requested = 0;
                drop(inner);
                self.poll()?;
            }
            Ok(to_write)
        }

        fn flush(&mut self) -> io::Result<()> {
            // No-op for synchronous writers.
            Ok(())
        }
    }

    pub(crate) fn push_mode<'a, P, W>(
        processor: P,
        writer: W,
        buffer_capacity: usize,
    ) -> impl io::Write + 'a
    where
        P: super::DataProcessor + 'a,
        W: io::Write + Unpin + 'a,
    {
        let inner = Rc::new(RefCell::new(Inner {
            buffer: VecDeque::with_capacity(buffer_capacity),
            closed: false,
            bytes_requested: 0,
        }));
        let reader = InvertedReader {
            inner: inner.clone(),
        };
        let future = async move {
            processor
                .process(reader, super::SyncAdapter { inner: writer })
                .await
        };

        Writer {
            reader_state: inner,
            polled_future: Some(Box::pin(future)),
        }
    }
}

mod inv_writer {
    use std::{
        cell::RefCell,
        collections::VecDeque,
        io,
        pin::Pin,
        rc::Rc,
        task::{Context, Poll, Waker},
    };

    use bytes::Buf as _;
    use futures::AsyncWrite;

    struct Inner {
        buffer: VecDeque<u8>,
        closed: bool,
        space_requested: usize,
    }

    impl Inner {
        fn remaining_capacity(&self) -> usize {
            self.buffer.capacity() - self.buffer.len()
        }
    }

    struct InvertedWriter {
        inner: Rc<RefCell<Inner>>,
    }

    impl AsyncWrite for InvertedWriter {
        fn poll_write(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            let mut inner = self.inner.borrow_mut();
            if inner.closed {
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "Writer attempted to write to closed reader",
                )));
            }
            let remaining_capacity = inner.buffer.capacity() - inner.buffer.len();
            let to_write = std::cmp::min(buf.len(), remaining_capacity);
            if to_write > 0 {
                inner.buffer.extend(&buf[..to_write]);
                Poll::Ready(Ok(to_write))
            } else {
                // We don't have room to write any data, so we need to wait
                // for the reader to consume some data.
                inner.space_requested = std::cmp::min(buf.len(), remaining_capacity);
                Poll::Pending
            }
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            // Flush means we want to empty the current buffer. Returning pending
            // will cause the caller to try again later, which is what we want.
            Poll::Pending
        }

        fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            let mut inner = self.inner.borrow_mut();
            inner.closed = true;
            Poll::Ready(Ok(()))
        }
    }

    pub(super) struct Reader<'a> {
        writer_state: Rc<RefCell<Inner>>,
        polled_future: Option<Pin<Box<dyn futures::Future<Output = io::Result<()>> + 'a>>>,
    }

    impl Reader<'_> {
        fn poll(&mut self) -> io::Result<()> {
            let mut cx = Context::from_waker(Waker::noop());
            let Some(fut) = &mut self.polled_future else {
                // Polling after completion is conceptually fine. The buffer
                // won't grow again, but that's not an error.
                return Ok(());
            };
            match fut.as_mut().poll(&mut cx) {
                Poll::Ready(Ok(())) => {
                    // The future has completed. All of the data it intended to
                    // write should be in the buffer now.
                    let mut inner = self.writer_state.borrow_mut();
                    inner.closed = true;
                    self.polled_future = None;
                }
                Poll::Ready(Err(e)) => return Err(e),
                Poll::Pending => {}
            }
            Ok(())
        }
    }

    impl io::Read for Reader<'_> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let mut inner = self.writer_state.borrow_mut();
            if inner.buffer.is_empty() {
                if inner.closed {
                    return Ok(0);
                }
                drop(inner);
                self.poll()?;
                inner = self.writer_state.borrow_mut();
            }
            let to_read = std::cmp::min(buf.len(), inner.buffer.len());
            // If the buffer is empty, either we're done, or we need to wait
            // for more data.
            if to_read == 0 {
                assert!(!buf.is_empty());
                assert!(inner.closed);
                return Ok(0);
            }
            inner.buffer.copy_to_slice(&mut buf[..to_read]);
            if inner.remaining_capacity() >= inner.space_requested {
                // If we've satisfied the requested space, clear the request
                // so that the writer can proceed.
                inner.space_requested = 0;
                drop(inner);
                self.poll()?;
            }
            Ok(to_read)
        }
    }

    pub(crate) fn pull_mode<'a, P, R>(
        processor: P,
        reader: R,
        buffer_capacity: usize,
    ) -> impl io::Read + 'a
    where
        P: super::DataProcessor + 'a,
        R: io::Read + Unpin + 'a,
    {
        let inner = Rc::new(RefCell::new(Inner {
            buffer: VecDeque::with_capacity(buffer_capacity),
            closed: false,
            space_requested: 0,
        }));
        let writer = InvertedWriter {
            inner: inner.clone(),
        };
        let future = async move {
            processor
                .process(super::SyncAdapter { inner: reader }, writer)
                .await
        };

        Reader {
            writer_state: inner,
            polled_future: Some(Box::pin(future)),
        }
    }
}
