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

    #[allow(dead_code, reason = "Will be used in lazy block")]
    fn pull<'a, R>(self, reader: R, buffer_capacity: usize) -> Reader<'a>
    where
        Self: Sized + 'static,
        R: io::Read + Unpin + 'a,
    {
        inv_writer::pull_mode(self, reader, buffer_capacity)
    }

    fn push<'a, W>(self, writer: W, buffer_capacity: usize) -> Writer<'a>
    where
        Self: Sized + 'a,
        W: io::Write + Unpin + 'a,
    {
        inv_reader::push_mode(self, writer, buffer_capacity)
    }
}

/// The input type for the continuation channel.
struct DataReady;

/// The output type for the continuation channel.
struct DataNeeded {
    /// The amount of data needed by the operation.
    ///
    /// The meaning of this value is context-dependent. For a reader, it is
    /// the number of bytes the reader would like to have available to read.
    /// For a writer, it is the amount of space the writer would like to have
    /// available to write.
    requested_data_size: usize,
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
        rc::Rc,
        task::{Context, Poll},
    };

    use bytes::Buf as _;
    use futures::{AsyncRead, FutureExt as _};

    use crate::utils::continuation::{Channel, ChannelYield, Continuation, ContinuationResult};

    struct Inner {
        buffer: VecDeque<u8>,
        closed: bool,
    }

    struct InvertedReader {
        channel: Channel<super::DataNeeded, super::DataReady>,
        inner: Rc<RefCell<Inner>>,
        read_op: Option<ChannelYield<super::DataNeeded, super::DataReady>>,
    }

    impl AsyncRead for InvertedReader {
        fn poll_read(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<io::Result<usize>> {
            let this = &mut *self;
            loop {
                let read_op = if let Some(read_op) = &mut this.read_op {
                    read_op
                } else {
                    let mut inner = this.inner.borrow_mut();
                    if buf.is_empty() {
                        // Nothing to do.
                        return Poll::Ready(Ok(0));
                    }

                    let to_read = std::cmp::min(buf.len(), inner.buffer.len());

                    if to_read > 0 {
                        // Yield data if we have any left.
                        inner.buffer.copy_to_slice(&mut buf[..to_read]);
                        return Poll::Ready(Ok(to_read));
                    }

                    if inner.closed {
                        // No more data, and the writer is closed. Return 0.
                        return Poll::Ready(Ok(0));
                    }

                    // We need more data. Request it from the writer.
                    this.read_op = Some(this.channel.yield_value(super::DataNeeded {
                        requested_data_size: std::cmp::min(buf.len(), inner.buffer.capacity()),
                    }));
                    this.read_op.as_mut().unwrap()
                };

                std::task::ready!(read_op.poll_unpin(cx));
                this.read_op = None;
                // If this returns, then we should have data available now.
            }
        }
    }

    pub(crate) struct Writer<'a> {
        reader_state: Rc<RefCell<Inner>>,
        continuation: Continuation<'a, super::DataReady, super::DataNeeded, io::Result<()>>,
        bytes_requested: usize,
    }

    impl Writer<'_> {
        fn pump_continuation(&mut self) -> ContinuationResult<super::DataNeeded, io::Result<()>> {
            if self.continuation.is_finished() {
                // We're done.
                return ContinuationResult::Complete(Ok(()));
            }
            if self.continuation.has_started() {
                self.continuation.next(super::DataReady)
            } else {
                self.continuation.start()
            }
        }
        /// Closes the writer, ensuring that all data is flushed to the reader.
        pub(crate) fn close(mut self) -> io::Result<()> {
            {
                let mut inner = self.reader_state.borrow_mut();
                assert!(!inner.closed);
                inner.closed = true;
            }
            match self.pump_continuation() {
                ContinuationResult::Yield(_) => {
                    // Since we ran while closed, the channel should not be
                    // yielding back to us. Instead, it should internally be
                    // returning 0 from the internal reader.
                    unreachable!("Writer continuation yielded after close");
                }
                ContinuationResult::Complete(r) => r,
            }
        }
    }

    impl Drop for Writer<'_> {
        fn drop(&mut self) {
            if self.continuation.is_finished() {
                // We have already completed, so nothing more we can do.
                return;
            }
            {
                let mut inner = self.reader_state.borrow_mut();
                if inner.closed {
                    // We're already closed, so nothing more we can do.
                    return;
                }
                inner.closed = true;
            }
            match self.pump_continuation() {
                ContinuationResult::Yield(_) => {
                    // This shouldn't happen after we've set the closed flag.
                    unreachable!("Requested more data after writer closed");
                }
                ContinuationResult::Complete(_) => {
                    // We're done. We don't care about the result, since
                    // we're in a destructor.
                }
            }
        }
    }

    impl io::Write for Writer<'_> {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            if buf.is_empty() {
                return Ok(0);
            }

            if self.continuation.is_finished() {
                // If the continuation is finished, then we have no more
                // data to write.
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "Writer attempted to write to closed reader",
                ));
            }

            let bytes_written;
            let curr_buffer_len;
            {
                let mut inner = self.reader_state.borrow_mut();
                if inner.closed {
                    return Err(io::Error::new(
                        io::ErrorKind::BrokenPipe,
                        "Writer attempted to write to closed reader",
                    ));
                }
                let remaining_capacity = inner.buffer.capacity() - inner.buffer.len();
                bytes_written = std::cmp::min(buf.len(), remaining_capacity);
                inner.buffer.extend(&buf[..bytes_written]);
                curr_buffer_len = inner.buffer.len();
            }
            if curr_buffer_len >= self.bytes_requested {
                match self.pump_continuation() {
                    ContinuationResult::Yield(data_needed) => {
                        self.bytes_requested = data_needed.requested_data_size;
                    }
                    ContinuationResult::Complete(Err(e)) => {
                        return Err(e);
                    }
                    ContinuationResult::Complete(Ok(())) => {}
                }
            }
            Ok(bytes_written)
        }

        fn flush(&mut self) -> io::Result<()> {
            // Ensure all of the buffer is written.
            {
                let guard = self.reader_state.borrow_mut();
                if guard.buffer.is_empty() {
                    return Ok(());
                }
            }
            // If there's anything in the buffer, then write() must have been
            // called, so the continuation must have been started.
            assert!(self.continuation.has_started());
            match self.pump_continuation() {
                ContinuationResult::Yield(data_needed) => {
                    self.bytes_requested = data_needed.requested_data_size;
                    let inner = self.reader_state.borrow();
                    assert!(inner.buffer.is_empty());
                    Ok(())
                }
                ContinuationResult::Complete(r) => r,
            }
        }
    }

    pub(crate) fn push_mode<'a, P, W>(processor: P, writer: W, buffer_capacity: usize) -> Writer<'a>
    where
        P: super::DataProcessor + 'a,
        W: io::Write + Unpin + 'a,
    {
        let inner = Rc::new(RefCell::new(Inner {
            buffer: VecDeque::with_capacity(buffer_capacity),
            closed: false,
        }));

        let cont = Continuation::new({
            let inner = inner.clone();
            async move |channel: Channel<super::DataNeeded, super::DataReady>| {
                let reader = InvertedReader {
                    channel,
                    inner,
                    read_op: None,
                };

                processor
                    .process(reader, super::SyncAdapter { inner: writer })
                    .await
            }
        });

        Writer {
            reader_state: inner,
            continuation: cont,
            bytes_requested: 0,
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
        task::{Context, Poll},
    };

    use futures::{AsyncWrite, FutureExt as _};

    use crate::utils::continuation::{Channel, ChannelYield, Continuation, ContinuationResult};

    struct Inner {
        buffer: VecDeque<u8>,
        closed: bool,
    }

    impl Inner {
        fn remaining_capacity(&self) -> usize {
            self.buffer.capacity() - self.buffer.len()
        }
    }

    struct InvertedWriter {
        channel: Channel<super::DataNeeded, super::DataReady>,
        inner: Rc<RefCell<Inner>>,
        yield_op: Option<ChannelYield<super::DataNeeded, super::DataReady>>,
    }

    impl AsyncWrite for InvertedWriter {
        fn poll_write(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            if buf.is_empty() {
                return Poll::Ready(Ok(0));
            }

            let this = self.get_mut();
            loop {
                if let Some(write_op) = &mut this.yield_op {
                    // We have a pending write operation. Poll it to completion.
                    std::task::ready!(write_op.poll_unpin(cx));
                    this.yield_op = None;
                }
                let mut inner = this.inner.borrow_mut();

                if inner.closed {
                    return Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::BrokenPipe,
                        "Writer attempted to write to closed reader",
                    )));
                }

                let to_write = std::cmp::min(buf.len(), inner.remaining_capacity());
                if to_write > 0 {
                    inner.buffer.extend(&buf[..to_write]);
                    return Poll::Ready(Ok(to_write));
                }

                // No room to write any data. Send a request for more space.
                this.yield_op = Some(this.channel.yield_value(super::DataNeeded {
                    requested_data_size: std::cmp::min(buf.len(), inner.buffer.capacity()),
                }));
            }
        }

        fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            // Flush means we want to empty the current buffer. Returning pending
            // will cause the caller to try again later, which is what we want.
            loop {
                if let Some(yield_op) = &mut self.yield_op {
                    // We have a pending write operation. Poll it to completion.
                    std::task::ready!(yield_op.poll_unpin(cx));
                    self.yield_op = None;
                }

                let capacity = {
                    let inner = self.inner.borrow();
                    if inner.buffer.is_empty() {
                        return Poll::Ready(Ok(()));
                    }
                    inner.buffer.capacity()
                };

                self.yield_op = Some(self.channel.yield_value(super::DataNeeded {
                    requested_data_size: capacity,
                }));
            }
        }

        fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            let mut inner = self.inner.borrow_mut();
            inner.closed = true;
            Poll::Ready(Ok(()))
        }
    }

    pub(crate) struct Reader<'a> {
        writer_state: Rc<RefCell<Inner>>,
        continuation: Continuation<'a, super::DataReady, super::DataNeeded, io::Result<()>>,
    }

    impl Reader<'_> {
        #[allow(dead_code, reason = "Might be used soon")]
        pub(crate) fn close(mut self) -> io::Result<()> {
            // Technically, it's safe to drop the reader, but make a reasonable
            // attempt in case the decoder has *shudder* side effects...
            {
                self.writer_state.borrow_mut().closed = true;
            }

            if self.continuation.is_finished() {
                // We're good to go.
                return Ok(());
            }
            let cont_result = if self.continuation.has_started() {
                self.continuation.next(super::DataReady)
            } else {
                self.continuation.start()
            };
            match cont_result {
                ContinuationResult::Yield(_) => {
                    // This shouldn't happen after we've set the closed flag.
                    unreachable!("Requested more data after reader closed.");
                }
                ContinuationResult::Complete(result) => result,
            }
        }
    }

    impl Drop for Reader<'_> {
        fn drop(&mut self) {
            if std::thread::panicking() {
                // Never cause a double-panic
                return;
            }
            let guard = self.writer_state.borrow();
            assert!(
                guard.closed,
                "Must always close the reader before dropping."
            );
        }
    }

    impl io::Read for Reader<'_> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if buf.is_empty() {
                return Ok(0);
            }

            loop {
                {
                    let mut inner = self.writer_state.borrow_mut();
                    if !inner.buffer.is_empty() {
                        let to_read = std::cmp::min(buf.len(), inner.buffer.len());
                        for (dst, src) in
                            buf[..to_read].iter_mut().zip(inner.buffer.drain(..to_read))
                        {
                            *dst = src;
                        }
                        return Ok(to_read);
                    }

                    if inner.closed {
                        return Ok(0);
                    }
                }

                if self.continuation.is_finished() {
                    // If the continuation is finished, then we have no more
                    // data to read.
                    return Ok(0);
                }
                // If we're here, we have a valid read, but the buffer is empty.
                // Pump the continuation to get more data.
                let cont_result = if self.continuation.has_started() {
                    self.continuation.next(super::DataReady)
                } else {
                    self.continuation.start()
                };

                match cont_result {
                    ContinuationResult::Yield(_) => {
                        // The continuation yielded, so the buffer should now
                        // have data available.
                        //
                        // We ignore the parameter, as this approach simply
                        // yields each time new data is necessary.
                    }
                    ContinuationResult::Complete(result) => {
                        result?;
                        return Ok(0);
                    }
                }
            }
        }
    }

    pub(crate) fn pull_mode<'a, P, R>(processor: P, reader: R, buffer_capacity: usize) -> Reader<'a>
    where
        P: super::DataProcessor + 'a,
        R: io::Read + Unpin + 'a,
    {
        let inner = Rc::new(RefCell::new(Inner {
            buffer: VecDeque::with_capacity(buffer_capacity),
            closed: false,
        }));
        let cont = Continuation::new({
            let inner = inner.clone();
            async move |channel: Channel<super::DataNeeded, super::DataReady>| {
                let writer = InvertedWriter {
                    channel,
                    inner,
                    yield_op: None,
                };

                processor
                    .process(super::SyncAdapter { inner: reader }, writer)
                    .await
            }
        });

        Reader {
            writer_state: inner,
            continuation: cont,
        }
    }
}

pub(crate) use inv_reader::Writer;
pub(crate) use inv_writer::Reader;

#[cfg(test)]
mod tests {
    use super::*;
    use futures::prelude::*;
    use proptest::prelude::*;

    struct IdentityProcessor;

    impl DataProcessor for IdentityProcessor {
        async fn process<R, W>(self, mut reader: R, mut writer: W) -> io::Result<()>
        where
            R: AsyncRead + Unpin,
            W: AsyncWrite + Unpin,
        {
            let mut buf = [0u8; 1024];
            loop {
                let n = reader.read(&mut buf).await?;
                if n == 0 {
                    break;
                }
                writer.write_all(&buf[..n]).await?;
            }
            writer.flush().await?;
            Ok(())
        }
    }

    #[test]
    fn test_reader_writer() {
        proptest!(|(data in prop::collection::vec(any::<u8>(), 0..10000))| {
            let mut output = Vec::new();
            {
                let mut source = IdentityProcessor.pull(io::Cursor::new(data.clone()), 1024);
                let mut sink = io::Cursor::new(&mut output);
                std::io::copy(&mut source, &mut sink)?;
                source.close()?;
            }
            prop_assert_eq!(data, output);
        });

        proptest!(|(data in prop::collection::vec(any::<u8>(), 0..10000))| {
            let mut output = Vec::new();
            {
                let mut sink = IdentityProcessor.push(io::Cursor::new(&mut output), 1024);
                std::io::copy(&mut io::Cursor::new(data.clone()), &mut sink)?;
                sink.close()?;
            }
            prop_assert_eq!(data, output);
        });
    }
}
