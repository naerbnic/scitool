use std::{future::Future, net::SocketAddr};

use futures::TryFutureExt;
use tokio::net::TcpStream;
use tokio::task::JoinHandle;

pub async fn start_tcp<F, Fut, R>(
    timeout: tokio::time::Instant,
    body: F,
) -> anyhow::Result<(SocketAddr, JoinHandle<anyhow::Result<R>>)>
where
    F: FnOnce(TcpStream) -> Fut + Send + 'static,
    Fut: Future<Output = anyhow::Result<R>> + Send,
    R: Send + 'static,
{
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let local_addr = listener.local_addr()?;
    log::debug!("Listening on {}", local_addr);

    let accept = tokio::time::timeout_at(timeout, async move {
        let (stream, _) = listener.accept().await?;
        log::info!(
            "{}: Accepted connection from {}",
            local_addr,
            stream.peer_addr()?
        );
        Ok::<_, anyhow::Error>(stream)
    })
    .map_err(anyhow::Error::from);

    let task = tokio::spawn(async move {
        let stream = accept.await??;
        body(stream).await
    });
    Ok((local_addr, task))
}
