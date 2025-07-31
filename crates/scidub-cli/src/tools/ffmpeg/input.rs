use std::{ffi::OsString, net::SocketAddr, path::Path};

use super::tcp;

/// A trait that maintains state for an `FFMpeg` input.
///
/// Returns the URL of the input. This object should be alive during the
/// lifetime of the `FFMpeg` process.
pub trait InputState {
    fn url(&self) -> OsString;
    fn wait(self) -> impl std::future::Future<Output = anyhow::Result<()>>;
}

struct SimpleInputState(OsString);

impl InputState for SimpleInputState {
    fn url(&self) -> OsString {
        self.0.clone()
    }
    async fn wait(self) -> anyhow::Result<()> {
        Ok(())
    }
}

struct TcpInputState {
    /// Thread handling the TCP connection.
    task: tokio::task::JoinHandle<anyhow::Result<()>>,
    /// URL of the input.
    local_addr: SocketAddr,
}

impl TcpInputState {
    async fn new<R: tokio::io::AsyncRead + Send + Unpin + 'static>(
        mut read: R,
        timeout: std::time::Instant,
    ) -> anyhow::Result<Self> {
        let (local_addr, task) = tcp::start_tcp(timeout, async move |mut stream| {
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
        self.task.await??;
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
pub struct ReaderInput<R>(R);

impl<R> ReaderInput<R>
where
    R: tokio::io::AsyncRead + Send + Unpin + 'static,
{
    pub fn new(reader: R) -> Self {
        Self(reader)
    }
}

impl<R> Input for ReaderInput<R>
where
    R: tokio::io::AsyncRead + Clone + Send + Unpin + 'static,
{
    async fn create_state(self) -> anyhow::Result<impl InputState> {
        TcpInputState::new(
            self.0,
            std::time::Instant::now() + std::time::Duration::from_millis(100),
        )
        .await
    }
}
