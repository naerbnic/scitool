use std::{ffi::OsString, net::SocketAddr, path::Path};

use super::tcp;

pub trait OutputState {
    type OutputType;
    fn url(&self) -> OsString;
    fn wait(self) -> impl std::future::Future<Output = anyhow::Result<Self::OutputType>>;
}

struct SimpleOutputState(OsString);

impl OutputState for SimpleOutputState {
    type OutputType = ();
    fn url(&self) -> OsString {
        self.0.clone()
    }
    async fn wait(self) -> anyhow::Result<()> {
        Ok(())
    }
}

struct TcpOutputState {
    /// Thread handling the TCP connection.
    task: tokio::task::JoinHandle<anyhow::Result<Vec<u8>>>,
    /// URL of the input.
    local_addr: SocketAddr,
}

impl TcpOutputState {
    async fn new(timeout: std::time::Instant) -> anyhow::Result<Self> {
        let (local_addr, task) = tcp::start_tcp(timeout, {
            async move |mut stream| {
                let mut output = Vec::new();
                tokio::io::copy(&mut stream, &mut output).await?;
                Ok(output)
            }
        })
        .await?;
        Ok(Self { task, local_addr })
    }
}

impl OutputState for TcpOutputState {
    type OutputType = Vec<u8>;
    fn url(&self) -> OsString {
        format!("tcp://{}", self.local_addr).into()
    }

    async fn wait(self) -> anyhow::Result<Vec<u8>> {
        self.task.await?
    }
}

pub trait Output {
    type OutputType;
    fn create_state(
        self,
    ) -> impl std::future::Future<
        Output = anyhow::Result<impl OutputState<OutputType = Self::OutputType>>,
    > + Send;
}

impl<T> Output for T
where
    T: AsRef<Path> + Send,
{
    type OutputType = ();
    async fn create_state(self) -> anyhow::Result<impl OutputState<OutputType = ()>> {
        Ok(SimpleOutputState(self.as_ref().as_os_str().to_owned()))
    }
}

pub struct VecOutput;
impl Output for VecOutput {
    type OutputType = Vec<u8>;
    async fn create_state(self) -> anyhow::Result<impl OutputState<OutputType = Vec<u8>>> {
        TcpOutputState::new(std::time::Instant::now() + std::time::Duration::from_secs(5)).await
    }
}
