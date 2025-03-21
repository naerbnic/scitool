use std::{ffi::OsString, net::SocketAddr, path::Path};

pub trait OutputState {
    fn url(&self) -> OsString;
    fn wait(self) -> impl Future<Output = anyhow::Result<()>>;
}

struct SimpleOutputState(OsString);

impl OutputState for SimpleOutputState {
    fn url(&self) -> OsString {
        self.0.to_os_string()
    }
    async fn wait(self) -> anyhow::Result<()> {
        Ok(())
    }
}

struct TcpOutputState {
    /// Thread handling the TCP connection.
    task: smol::Task<anyhow::Result<()>>,
    /// URL of the input.
    local_addr: SocketAddr,
}

impl TcpOutputState {
    async fn new<R: smol::io::AsyncWrite + Send + 'static>(
        write: R,
        timeout: std::time::Instant,
    ) -> anyhow::Result<Self> {
        let listener = smol::net::TcpListener::bind("127.0.0.1:0").await?;
        let local_addr = listener.local_addr()?;
        let timer = smol::Timer::at(timeout);
        let task = smol::spawn(async move {
            let stream = smol::future::or(
                async move {
                    let (stream, _) = listener.accept().await?;
                    Ok(stream)
                },
                async move {
                    timer.await;
                    Err(anyhow::anyhow!("Connection timed out."))
                },
            )
            .await?;
            smol::io::copy(stream, write).await?;
            Ok(())
        });
        Ok(Self { task, local_addr })
    }
}

impl OutputState for TcpOutputState {
    fn url(&self) -> OsString {
        format!("tcp://{}", self.local_addr).into()
    }

    async fn wait(self) -> anyhow::Result<()> {
        self.task.await?;
        Ok(())
    }
}

pub trait Output {
    fn create_state(
        self,
    ) -> impl std::future::Future<Output = anyhow::Result<impl OutputState>> + Send;
}

impl<T> Output for T
where
    T: AsRef<Path> + Send,
{
    async fn create_state(self) -> anyhow::Result<impl OutputState> {
        Ok(SimpleOutputState(self.as_ref().as_os_str().to_owned()))
    }
}

pub struct WriterOutput<W>(W);
impl<W> Output for WriterOutput<W>
where
    W: smol::io::AsyncWrite + Send + Unpin + 'static,
{
    async fn create_state(self) -> anyhow::Result<impl OutputState> {
        TcpOutputState::new(
            self.0,
            std::time::Instant::now() + std::time::Duration::from_secs(5),
        )
        .await
    }
}
