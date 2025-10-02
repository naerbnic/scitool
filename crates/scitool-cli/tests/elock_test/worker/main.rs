#![expect(clippy::mod_module_files)]

use crate::shared::{
    config::WorkerConfig,
    msg::{ManagerMessage, create_message_sink, create_message_stream},
};

use std::io::Error as IoError;

use futures::StreamExt as _;

use scitool_cli::fs::file_lock::ephemeral;

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
    let mut msg_input = create_message_stream(tokio::io::stdin());
    let mut _msg_output = create_message_sink(tokio::io::stdout());

    let config: WorkerConfig = serde_json::from_value(
        msg_input
            .next()
            .await
            .expect("Failed to read config")
            .expect("No config received"),
    )?;

    let mut msg_input = msg_input.map(|msg| {
        let msg = msg?;
        let msg = serde_json::from_value::<ManagerMessage>(msg)?;
        Ok::<_, IoError>(msg)
    });

    let lock_type = if config.use_shared {
        ephemeral::LockType::Shared
    } else {
        ephemeral::LockType::Exclusive
    };

    let (mut canceller, token) = cancel::Canceller::new();

    tokio::join!(
        async {
            while !token.is_cancelled() {
                let _lock = spawn_blocking_propagate({
                    let config = config.clone();
                    move || {
                        let start = std::time::Instant::now();
                        let lock = ephemeral::lock_file(&config.lock_file_path, lock_type)
                            .expect("Failed to acquire lock");
                        eprintln!("Lock acquired in {:?}", start.elapsed());
                        lock
                    }
                })
                .await;
                let duration = rand::random_range(config.hold_ms.min..config.hold_ms.max);
                // Do random work until stopped.
                eprintln!(
                    "Holding lock for {} ms (shared: {})",
                    duration.as_millis(),
                    config.use_shared
                );
                tokio::time::sleep(duration).await;
            }
        },
        async {
            eprintln!("Waiting for messages...");
            // Read any incoming messages (if needed)
            let msg = msg_input
                .next()
                .await
                .expect("Error in stream")
                .expect("Stream closed");
            match msg {
                ManagerMessage::Stop => {
                    eprintln!("Received stop signal, stopping work...");
                    canceller.cancel();
                }
            }
            eprintln!("Message reader exiting...");
        },
    );

    Ok(())
}
