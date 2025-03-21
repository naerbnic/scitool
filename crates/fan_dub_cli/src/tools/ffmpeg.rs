use std::{
    ffi::{OsStr, OsString},
    io::{BufRead, BufReader},
    path::Path,
    sync::Arc,
};

use smol::io::AsyncWriteExt;

struct CloseOnDropChildProcess {
    child: std::process::Child,
}

impl Drop for CloseOnDropChildProcess {
    fn drop(&mut self) {
        self.child.kill().expect("Failed to kill child process");
    }
}

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

struct PermanentEventState {
    waiters: Option<Vec<std::task::Waker>>,
}

impl PermanentEventState {
    fn new() -> Self {
        PermanentEventState {
            waiters: Some(Vec::new()),
        }
    }

    fn add_waiter(&mut self, waker: std::task::Waker) -> std::task::Poll<()> {
        if let Some(waiters) = &mut self.waiters {
            waiters.push(waker);
            std::task::Poll::Pending
        } else {
            std::task::Poll::Ready(())
        }
    }

    fn notify(&mut self) {
        if let Some(waiters) = self.waiters.take() {
            for waker in waiters {
                waker.wake();
            }
        }
    }
}

struct SyncReadStream<R> {
    thread: std::thread::JoinHandle<()>,
    cancel_event: PermanentEvent,
    reader: smol::channel::Receiver<R>,
}

struct PermanentEventInner {
    state: std::sync::Mutex<PermanentEventState>,
}

impl PermanentEventInner {
    fn new() -> Self {
        PermanentEventInner {
            state: std::sync::Mutex::new(PermanentEventState::new()),
        }
    }

    fn add_waiter(&self, waker: &std::task::Waker) -> std::task::Poll<()> {
        let mut state = self.state.lock().unwrap();
        state.add_waiter(waker.clone())
    }

    fn notify(&self) {
        let mut state = self.state.lock().unwrap();
        state.notify();
    }
}

/// A simple event that can be waited on as a future.
#[derive(Clone)]
struct PermanentEvent(Arc<PermanentEventInner>);

impl PermanentEvent {
    fn new() -> Self {
        PermanentEvent(Arc::new(PermanentEventInner::new()))
    }

    async fn wait(&self) {
        smol::future::poll_fn(|cx| self.0.add_waiter(cx.waker())).await
    }

    fn notify(&self) {
        self.0.notify();
    }
}

/// A trait that maintains state for an FFMpeg input.
///
/// Returns the URL of the input. This object should be alive during the
/// lifetime of the FFMpeg process.
trait InputState {
    fn url(&self) -> &OsStr;
}

struct SimpleInputState(OsString);

impl InputState for SimpleInputState {
    fn url(&self) -> &OsStr {
        &self.0
    }
}

struct TcpInputState {
    /// Thread handling the TCP connection.
    thread: std::thread::JoinHandle<()>,
    /// URL of the input.
    url: OsString,
}

impl TcpInputState {
    fn new<R: smol::io::AsyncRead + Send + 'static>(read: R) -> anyhow::Result<Self> {
        let listener = smol::block_on(smol::net::TcpListener::bind("127.0.0.1"))?;
        let local_addr = listener.local_addr()?;
        let url = format!("tcp://{}", local_addr).into();
        let cancel_event = PermanentEvent::new();
        let handle = std::thread::spawn({
            let cancel_event = cancel_event.clone();
            move || {
                smol::block_on(smol::future::or(cancel_event.wait(), async move {
                    let (mut stream, _) = listener.accept().await.unwrap();
                    smol::io::copy(read, &mut stream).await.unwrap();
                }))
            }
        });
        Ok(Self {
            thread: handle,
            url,
        })
    }
}

impl InputState for TcpInputState {
    fn url(&self) -> &OsStr {
        &self.url
    }
}

pub enum Input<'a> {
    File(&'a Path),
    Buffer(&'a [u8]),
}

impl<'a> Input<'a> {
    fn create_state<'b>(&'b self) -> Box<dyn InputState + 'b> {
        match self {
            Input::File(path) => Box::new(SimpleInputState(path.as_os_str().to_owned())),
            Input::Buffer(buffer) => {
                todo!()
            }
        }
    }
}

pub struct FfmpegTool {
    binary_path: std::path::PathBuf,
}

impl FfmpegTool {
    pub fn from_path(path: std::path::PathBuf) -> Self {
        FfmpegTool { binary_path: path }
    }

    pub fn convert(
        &self,
        input: &Path,
        output: &Path,
        progress: &mut dyn ProgressListener,
    ) -> anyhow::Result<()> {
        let mut command = std::process::Command::new(&self.binary_path);
        let child = command
            .arg("-nostdin")
            .arg("-progress")
            .arg("pipe:1")
            .arg("-i")
            .arg(input)
            .arg(output)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .spawn()?;
        let mut child = CloseOnDropChildProcess { child };
        let stdout = BufReader::new(child.child.stdout.as_mut().expect("Failed to create pipe."));
        let mut progress_info = Vec::new();
        for line in stdout.lines() {
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
        let status = child.child.wait()?;

        anyhow::ensure!(
            status.success(),
            "ffmpeg process exited with non-zero status: {}",
            status
        );

        Ok(())
    }
}
