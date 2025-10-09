#![expect(clippy::mod_module_files)]

use crate::shared::msg::{
    ManagerMessage, WorkerMessage, create_message_sink, create_message_stream,
};

use std::io::Error as IoError;

use crosslock::ephemeral;
use futures::{SinkExt as _, StreamExt as _};

// Shared module between manager and worker
#[path = "../shared/mod.rs"]
mod shared;

mod cancel;

async fn spawn_blocking_propagate<F, T>(f: F) -> T
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let (send, recv) = tokio::sync::oneshot::channel();
    let result = tokio::task::spawn_blocking(move || {
        drop(send.send(f()));
    })
    .await;
    if let Err(err) = result {
        match err.try_into_panic() {
            Ok(panic) => std::panic::resume_unwind(panic),
            Err(err) => {
                if err.is_cancelled() {
                    unreachable!("Impossible for the task to be cancelled")
                } else {
                    panic!("Blocking task failed: {err}")
                }
            }
        }
    }

    // The only way we should get here is if the task completed successfully.
    // It's possible that the task panicked after sending
    recv.await
        .expect("If we're here, the task must have completed successfully")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let msg_input = create_message_stream(tokio::io::stdin());
    let msg_output = create_message_sink(tokio::io::stdout());

    let mut msg_input = msg_input.map(|msg| {
        let msg = msg?;
        let msg = serde_json::from_value::<ManagerMessage>(msg)?;
        Ok::<_, IoError>(msg)
    });

    let msg_output = msg_output.with(async move |msg: WorkerMessage| {
        let msg = serde_json::to_value(msg)?;
        Ok::<_, IoError>(msg)
    });

    let ManagerMessage::Start(config) = msg_input
        .next()
        .await
        .expect("Failed to read config")
        .expect("No config received")
    else {
        panic!("First message must be a Start message with config");
    };

    let lock_type = if config.use_shared {
        ephemeral::LockType::Shared
    } else {
        ephemeral::LockType::Exclusive
    };

    let (mut canceller, token) = cancel::Canceller::new();
    let (send, mut recv) = tokio::sync::mpsc::channel::<WorkerMessage>(16);

    tokio::join!(
        async move {
            while !token.is_cancelled() {
                let (lock, lock_time, lock_start) = spawn_blocking_propagate({
                    let config = config.clone();
                    move || {
                        let start = std::time::Instant::now();
                        let lock = ephemeral::lock_file(&config.lock_file_path, lock_type)
                            .expect("Failed to acquire lock");
                        (lock, start.elapsed(), std::time::SystemTime::now())
                    }
                })
                .await;
                let expected_hold_time = rand::random_range(config.hold_ms.min..config.hold_ms.max);
                let deadline = tokio::time::Instant::now() + expected_hold_time;

                send.send(WorkerMessage::LockAcquired(
                    crate::shared::msg::LockAcquired {
                        lock_time,
                        expected_hold_time,
                        lock_start,
                    },
                ))
                .await
                .expect("Failed to send lock acquired message");

                tokio::time::sleep_until(deadline).await;

                let (unlock_time, lock_end) = spawn_blocking_propagate(move || {
                    let lock_end = std::time::SystemTime::now();
                    let start = std::time::Instant::now();
                    drop(lock);
                    (start.elapsed(), lock_end)
                })
                .await;

                send.send(WorkerMessage::LockReleased(
                    crate::shared::msg::LockReleased {
                        unlock_time,
                        lock_end,
                    },
                ))
                .await
                .expect("Failed to send lock released message");
            }
        },
        async move {
            let mut msg_output = std::pin::pin!(msg_output);
            while let Some(msg) = recv.recv().await {
                msg_output
                    .send(msg)
                    .await
                    .expect("Failed to send lock released message");
            }
        },
        async move {
            // Read any incoming messages (if needed)
            let msg = msg_input
                .next()
                .await
                .expect("Error in stream")
                .expect("Stream closed");
            match msg {
                ManagerMessage::Stop => {
                    canceller.cancel();
                }
                ManagerMessage::Start(_) => {
                    panic!("Received unexpected Start message after initialization");
                }
            }
        },
    );

    Ok(())
}
