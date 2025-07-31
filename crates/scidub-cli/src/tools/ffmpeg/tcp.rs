use std::net::SocketAddr;

use tokio::net::TcpStream;
use tokio::task::JoinHandle;

pub(crate) async fn start_tcp<F, Fut, R>(
    timeout: std::time::Instant,
    body: F,
) -> anyhow::Result<(SocketAddr, JoinHandle<anyhow::Result<R>>)>
where
    F: FnOnce(TcpStream) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = anyhow::Result<R>> + Send,
    R: Send + 'static,
{
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let local_addr = listener.local_addr()?;
    log::debug!("Listening on {local_addr}");

    let timer = tokio::time::sleep_until(timeout.into());

    let task = tokio::spawn(async move {
        let stream = tokio::select! {
            result = async {
                let (stream, _) = listener.accept().await?;
                log::info!(
                    "{}: Accepted connection from {}",
                    local_addr,
                    stream.peer_addr()?
                );
                Ok(stream)
            } => result,
            () = timer => {
                Err(anyhow::anyhow!("Connection timed out."))
            }
        }?;
        body(stream).await
    });
    Ok((local_addr, task))
}
