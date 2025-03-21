use std::{ffi::OsString, net::SocketAddr, path::Path};

use smol::{io::AsyncBufReadExt, stream::StreamExt};

pub trait ProgressListener {
    fn on_progress(&mut self, done: bool, progress_info: Vec<(String, String)>);
}

pub struct NullProgressListener;

impl ProgressListener for NullProgressListener {
    fn on_progress(&mut self, done: bool, progress_info: Vec<(String, String)>) {
        eprintln!("Progress: {:?}", progress_info);
        eprintln!("Done: {}", done);
    }
}

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
    task: smol::Task<anyhow::Result<()>>,
    /// URL of the input.
    local_addr: SocketAddr,
}

impl TcpInputState {
    async fn new<R: smol::io::AsyncRead + Send + 'static>(
        read: R,
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
            smol::io::copy(read, stream).await?;
            Ok(())
        });
        Ok(Self { task, local_addr })
    }
}

impl InputState for TcpInputState {
    fn url(&self) -> OsString {
        format!("tcp://{}", self.local_addr).into()
    }

    async fn wait(self) -> anyhow::Result<()> {
        self.task.await?;
        Ok(())
    }
}

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

pub trait Input {
    fn create_state(
        self,
    ) -> impl std::future::Future<Output = anyhow::Result<impl InputState>> + Send;
}

impl<T> Input for T
where
    T: AsRef<Path> + Send,
{
    async fn create_state(self) -> anyhow::Result<impl InputState> {
        Ok(SimpleInputState(self.as_ref().as_os_str().to_owned()))
    }
}

pub struct BytesInput<S>(S);

impl<S> Input for BytesInput<S>
where
    S: AsRef<[u8]> + Send + Unpin + 'static,
{
    async fn create_state(self) -> anyhow::Result<impl InputState> {
        TcpInputState::new(
            smol::io::Cursor::new(self.0),
            std::time::Instant::now() + std::time::Duration::from_secs(5),
        )
        .await
    }
}

pub struct ReaderInput<R>(R);

impl<R> ReaderInput<R>
where
    R: smol::io::AsyncRead + Send + Unpin + 'static,
{
    pub fn new(reader: R) -> Self {
        Self(reader)
    }
}

impl<R> Input for ReaderInput<R>
where
    R: smol::io::AsyncRead + Send + Unpin + 'static,
{
    async fn create_state(self) -> anyhow::Result<impl InputState> {
        TcpInputState::new(
            self.0,
            std::time::Instant::now() + std::time::Duration::from_millis(100),
        )
        .await
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

pub struct FfmpegTool {
    binary_path: std::path::PathBuf,
}

impl FfmpegTool {
    pub fn from_path(path: std::path::PathBuf) -> Self {
        FfmpegTool { binary_path: path }
    }

    pub async fn convert<I, O>(
        &self,
        input: I,
        output: O,
        progress: &mut dyn ProgressListener,
    ) -> anyhow::Result<()>
    where
        I: Input,
        O: Output,
    {
        let mut command = smol::process::Command::new(&self.binary_path);
        let input_state = input.create_state().await?;
        let output_state = output.create_state().await?;
        let mut child = command
            .arg("-nostdin")
            .arg("-progress")
            .arg("pipe:1")
            .arg("-i")
            .arg(input_state.url())
            .arg(output_state.url())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .spawn()?;
        let stdout =
            smol::io::BufReader::new(child.stdout.as_mut().expect("Failed to create pipe."));
        let mut progress_info = Vec::new();
        let mut lines = stdout.lines();
        while let Some(line) = lines.next().await {
            let line = line?;
            if let Some(eq_index) = line.find('=') {
                let key = &line[..eq_index];
                let value = &line[eq_index + 1..];
                if key == "progress" {
                    let done = value.trim() == "end";
                    progress.on_progress(done, progress_info);
                    progress_info = Vec::new();
                } else {
                    let value = value.trim().to_string();
                    progress_info.push((key.to_string(), value));
                }
            }
        }
        let (status, _) = futures::join!(child.status(), input_state.wait(),);
        let status = status?;

        anyhow::ensure!(
            status.success(),
            "ffmpeg process exited with non-zero status: {}",
            status
        );

        Ok(())
    }
}
