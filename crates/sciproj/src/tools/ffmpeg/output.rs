use std::{ffi::OsString, net::SocketAddr, path::Path};

use tokio::sync::oneshot;

use super::tcp;
use crate::{
    helpers::futures::Spawned,
    imp::futures::{BoxFuture, prelude::*},
};

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
    task: BoxFuture<'static, anyhow::Result<Vec<u8>>>,
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
        self.task.await
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

struct TcpReaderOutputState {
    local_addr: SocketAddr,
    task: Spawned<anyhow::Result<()>>,
}

impl OutputState for TcpReaderOutputState {
    type OutputType = ();

    fn url(&self) -> OsString {
        format!("tcp://{}", self.local_addr).into()
    }

    async fn wait(self) -> anyhow::Result<()> {
        self.task.await
    }
}

pub(crate) struct TcpReaderOutput {
    task: Spawned<anyhow::Result<()>>,
    address_receiver: oneshot::Receiver<SocketAddr>,
}

impl TcpReaderOutput {
    pub(crate) fn new(timeout: std::time::Instant) -> (Self, impl AsyncRead) {
        let (send_address, recv_address) = oneshot::channel();
        let (send_stream, recv_stream) = oneshot::channel();
        let reader = tcp::Reader::new(recv_stream);
        let task = Spawned::spawn(async move {
            let stream = tcp::start_tcp_stream(timeout, send_address).await?;
            send_stream.send(stream).ok();
            Ok(())
        });
        (
            Self {
                task,
                address_receiver: recv_address,
            },
            reader,
        )
    }
}

impl Output for TcpReaderOutput {
    type OutputType = ();

    async fn create_state(self) -> anyhow::Result<impl OutputState<OutputType = ()>> {
        let local_addr = self.address_receiver.await?;
        Ok(TcpReaderOutputState {
            local_addr,
            task: self.task,
        })
    }
}
