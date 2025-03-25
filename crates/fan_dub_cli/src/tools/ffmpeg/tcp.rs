use std::{future::Future, net::SocketAddr};

use async_net::TcpStream;
use smol::Task;

pub async fn start_tcp<F, Fut, R>(
    timeout: std::time::Instant,
    body: F,
) -> anyhow::Result<(SocketAddr, Task<anyhow::Result<R>>)>
where
    F: FnOnce(TcpStream) -> Fut + Send + 'static,
    Fut: Future<Output = anyhow::Result<R>> + Send,
    R: Send + 'static,
{
    let listener = smol::net::TcpListener::bind("127.0.0.1:0").await?;
    let local_addr = listener.local_addr()?;
    eprintln!("Listening on {}", local_addr);

    let timer = smol::Timer::at(timeout);

    let task = smol::spawn(async move {
        let stream = smol::future::or(
            async move {
                let (stream, _) = listener.accept().await?;
                eprintln!(
                    "{}: Accepted connection from {}",
                    local_addr,
                    stream.peer_addr()?
                );
                Ok(stream)
            },
            async move {
                timer.await;
                Err(anyhow::anyhow!("Connection timed out."))
            },
        )
        .await?;
        body(stream).await
    });
    Ok((local_addr, task))
}
