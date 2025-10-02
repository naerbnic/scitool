use std::{
    io::{Error as IoError, Result as IoResult},
    pin::Pin,
    time::Duration,
};

use futures::{Sink, Stream};
use serde_json::Value;
use tokio::io::{
    AsyncBufReadExt as _, AsyncRead, AsyncWrite, AsyncWriteExt as _, BufReader, BufWriter,
};

use serde::{Deserialize, Serialize};

pub(crate) type BoxedMessageStream<T> = Pin<Box<dyn Stream<Item = IoResult<T>> + Send + 'static>>;

pub(crate) type BoxedMessageSink<T> = Pin<Box<dyn Sink<T, Error = IoError> + Send>>;

pub(crate) fn create_message_stream<R>(reader: R) -> BoxedMessageStream<Value>
where
    R: AsyncRead + Send + 'static,
{
    Box::pin(futures::stream::try_unfold(
        Box::pin(BufReader::new(reader)),
        |mut reader| async move {
            let mut line = Vec::new();
            let read_bytes = reader.read_until(b'\n', &mut line).await?;
            if read_bytes == 0 {
                return Ok(None);
            }
            let msg = serde_json::from_slice(&line)?;
            Ok(Some((msg, reader)))
        },
    ))
}

struct DropOnShutdownWriter<W>(Option<W>)
where
    W: AsyncWrite + Send + Sync + Unpin + 'static;

impl<W> DropOnShutdownWriter<W>
where
    W: AsyncWrite + Send + Sync + Unpin + 'static,
{
    fn new(writer: W) -> Self {
        Self(Some(writer))
    }

    fn writer_mut(&mut self) -> IoResult<&mut W> {
        self.0
            .as_mut()
            .ok_or_else(|| IoError::other("Writer already shut down"))
    }
}

impl<W> AsyncWrite for DropOnShutdownWriter<W>
where
    W: AsyncWrite + Send + Sync + Unpin + 'static,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        Pin::new(self.get_mut().writer_mut()?).poll_write(cx, buf)
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        Pin::new(self.get_mut().writer_mut()?).poll_flush(cx)
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        if let Some(writer) = self.get_mut().0.as_mut() {
            Pin::new(writer).poll_shutdown(cx)
        } else {
            std::task::Poll::Ready(Ok(()))
        }
    }
}

impl<W> Drop for DropOnShutdownWriter<W>
where
    W: AsyncWrite + Send + Sync + Unpin + 'static,
{
    fn drop(&mut self) {
        if let Some(mut writer) = self.0.take() {
            drop(tokio::spawn(async move { writer.shutdown().await }));
        }
    }
}

pub(crate) fn create_message_sink<W>(writer: W) -> BoxedMessageSink<Value>
where
    W: AsyncWrite + Send + Sync + Unpin + 'static,
{
    Box::pin(futures::sink::unfold(
        BufWriter::new(DropOnShutdownWriter::new(writer)),
        async move |mut writer, item: Value| {
            let msg_bytes = serde_json::to_vec(&item)?;
            writer.write_all(&msg_bytes).await?;
            writer.write_all(b"\n").await?;
            writer.flush().await?;
            Ok(writer)
        },
    ))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum ManagerMessage {
    Stop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LockAcquired {
    pub lock_time: Duration,
    pub expected_hold_time: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LockReleased {
    pub unlock_time: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum WorkerMessage {
    LockAcquired(LockAcquired),
    LockReleased(LockReleased),
}
