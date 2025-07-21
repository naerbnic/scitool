use std::{ffi::OsString, net::SocketAddr, path::Path, pin::pin};

use tokio::{io::AsyncRead, task::JoinHandle};

use super::tcp;

/// A trait that maintains state for an FFMpeg input.
///
/// Returns the URL of the input. This object should be alive during the
/// lifetime of the FFMpeg process.
pub trait InputState {
    fn url(&self) -> OsString;
    fn wait(self) -> impl Future<Output = anyhow::Result<()>>;
}

struct SimpleInputState(OsString);

impl InputState for SimpleInputState {
    fn url(&self) -> OsString {
        self.0.to_os_string()
    }
    async fn wait(self) -> anyhow::Result<()> {
        Ok(())
    }
}

struct TcpInputState {
    /// Thread handling the TCP connection.
    task: JoinHandle<anyhow::Result<()>>,
    /// URL of the input.
    local_addr: SocketAddr,
}

impl TcpInputState {
    async fn new<R: AsyncRead + Send + 'static>(
        read: R,
        timeout: tokio::time::Instant,
    ) -> anyhow::Result<Self> {
        let (local_addr, task) = tcp::start_tcp(timeout, async move |mut stream| {
            let mut read = pin!(read);
            tokio::io::copy(&mut read, &mut stream).await?;
            Ok(())
        })
        .await?;
        Ok(Self { task, local_addr })
    }
}

impl InputState for TcpInputState {
    fn url(&self) -> OsString {
        format!("tcp://{}", self.local_addr).into()
    }

    async fn wait(self) -> anyhow::Result<()> {
        let _ = self.task.await?;
        Ok(())
    }
}

pub trait Input: Clone {
    fn create_state(
        self,
    ) -> impl std::future::Future<Output = anyhow::Result<impl InputState>> + Send;
}

impl<T> Input for T
where
    T: AsRef<Path> + Clone + Send,
{
    async fn create_state(self) -> anyhow::Result<impl InputState> {
        Ok(SimpleInputState(self.as_ref().as_os_str().to_owned()))
    }
}

#[derive(Clone)]
pub struct BytesInput<S>(S);

impl<S> Input for BytesInput<S>
where
    S: AsRef<[u8]> + Clone + Send + Unpin + 'static,
{
    async fn create_state(self) -> anyhow::Result<impl InputState> {
        TcpInputState::new(
            std::io::Cursor::new(self.0),
            tokio::time::Instant::now() + std::time::Duration::from_secs(5),
        )
        .await
    }
}

#[derive(Clone)]
pub struct ReaderInput<R>(R);

impl<R> ReaderInput<R>
where
    R: AsyncRead + Send + Unpin + 'static,
{
    pub fn new(reader: R) -> Self {
        Self(reader)
    }
}

impl<R> Input for ReaderInput<R>
where
    R: AsyncRead + Clone + Send + Unpin + 'static,
{
    async fn create_state(self) -> anyhow::Result<impl InputState> {
        TcpInputState::new(
            self.0,
            tokio::time::Instant::now() + tokio::time::Duration::from_millis(100),
        )
        .await
    }
}
