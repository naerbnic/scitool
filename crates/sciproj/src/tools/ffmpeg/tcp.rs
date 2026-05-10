use std::{
    net::SocketAddr,
    pin::Pin,
    task::{Context, ready},
};

use tokio::net::TcpStream;

use crate::{
    helpers::futures::{SenderExt as _, Spawned},
    imp::futures::{self, BoxFuture, prelude::*, sync::oneshot},
};

pub(crate) async fn start_tcp<F, Fut, R>(
    timeout: std::time::Instant,
    body: F,
) -> anyhow::Result<(SocketAddr, BoxFuture<'static, anyhow::Result<R>>)>
where
    F: FnOnce(TcpStream) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = anyhow::Result<R>> + Send,
    R: Send + 'static,
{
    let (addr_tx, addr_rx) = oneshot::channel();

    let task = Spawned::spawn(async move {
        let stream = start_tcp_stream(timeout, addr_tx).await?;
        let result = body(stream).await?;
        Ok(result)
    });

    let local_addr = addr_rx.await?;

    Ok((local_addr, task.boxed()))
}

#[pin_project::pin_project(project = ReaderProj)]
enum ReaderState<R> {
    Waiting(#[pin] oneshot::Receiver<R>),
    Reading(#[pin] R),
}

#[pin_project::pin_project(project_replace)]
pub(crate) struct Reader<R> {
    #[pin]
    state: ReaderState<R>,
}

impl<R: AsyncRead> Reader<R> {
    pub(crate) fn new(recv: oneshot::Receiver<R>) -> Self {
        Self {
            state: ReaderState::Waiting(recv),
        }
    }
}

impl<R: AsyncRead> AsyncRead for Reader<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut futures::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        // A loop to make it easy to resume on the next state.
        loop {
            let proj = self.as_mut().project();
            match proj.state.project() {
                ReaderProj::Waiting(pin) => {
                    let oneshot_result = ready!(pin.poll(cx));
                    // If the oneshot was dropped, then we return an IO error.
                    let stream = oneshot_result.map_err(|_| {
                        std::io::Error::new(
                            std::io::ErrorKind::ConnectionAborted,
                            "Connection aborted upstream",
                        )
                    })?;
                    self.as_mut()
                        .project()
                        .state
                        .set(ReaderState::Reading(stream));
                    // Fallthrough to take a new value.
                }
                ReaderProj::Reading(pin) => return pin.poll_read(cx, buf),
            }
        }
    }
}

pub(crate) async fn start_tcp_stream(
    timeout: std::time::Instant,
    send_address: oneshot::Sender<SocketAddr>,
) -> anyhow::Result<TcpStream> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let local_addr = listener.local_addr()?;
    log::debug!("Listening on {local_addr}");

    send_address.send_and_forget(local_addr);

    let timer = tokio::time::sleep_until(timeout.into());

    let stream = tokio::select! {
        result = listener.accept() => {
            let (stream, _) = result?;
            log::info!(
                "{}: Accepted connection from {}",
                local_addr,
                stream.peer_addr()?
            );
            stream
        },
        () = timer => {
            return Err(std::io::Error::other("Connection timed out.").into());
        },
    };

    Ok(stream)
}
