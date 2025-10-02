#![expect(clippy::mod_module_files)]

use std::{
    io::{Error as IoError, Result as IoResult},
    process::Stdio,
    time::Duration,
};

use futures::{SinkExt, StreamExt, TryStreamExt};
use serde_json::Value;
use tokio::{
    sync::{Mutex as AsyncMutex, mpsc},
    task::JoinHandle,
};

use crate::shared::{
    config::{TimeRange, WorkerConfig},
    msg::{
        BoxedMessageSink, ManagerMessage, WorkerMessage, create_message_sink, create_message_stream,
    },
};

#[path = "../shared/mod.rs"]
mod shared;

struct ChildManager {
    child_proc: Option<tokio::process::Child>,
    msg_output: AsyncMutex<Option<BoxedMessageSink<Value>>>,
    worker_msg_task: Option<JoinHandle<IoResult<()>>>,
}

impl ChildManager {
    async fn spawn_new(
        id: usize,
        config: &WorkerConfig,
        sender: &mpsc::Sender<(usize, WorkerMessage)>,
    ) -> IoResult<Self> {
        let mut child_proc = tokio::process::Command::new(env!("CARGO_BIN_EXE_elock_test_worker"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let child_stdin = child_proc.stdin.take().expect("Failed to open stdin");
        let child_stdout = child_proc.stdout.take().expect("Failed to open stdout");
        let mut msg_output = create_message_sink(child_stdin);
        msg_output.send(serde_json::to_value(config)?).await?;
        let msg_input = create_message_stream(child_stdout);
        let worker_msg_task = tokio::spawn({
            let sender = sender.clone();
            async move {
                msg_input
                    .map(|msg| Ok(Some((id, serde_json::from_value::<WorkerMessage>(msg?)?))))
                    .try_for_each(async |msg| {
                        if let Some(msg) = msg {
                            let _ = sender.send(msg).await;
                        }
                        Ok(())
                    })
                    .await
            }
        });
        Ok(Self {
            child_proc: Some(child_proc),
            msg_output: AsyncMutex::new(Some(msg_output)),
            worker_msg_task: Some(worker_msg_task),
        })
    }

    async fn send(&self, msg: ManagerMessage) -> IoResult<()> {
        if let Some(msg_output) = self.msg_output.lock().await.as_mut() {
            msg_output.send(serde_json::to_value(msg)?).await?;
        }

        Ok(())
    }

    async fn close_send(&self) -> IoResult<()> {
        self.msg_output.lock().await.take();
        Ok(())
    }

    async fn join(mut self) -> IoResult<()> {
        // Shutdown writer to signal EOF to child
        self.close_send().await?;
        let mut child_proc = self.child_proc.take().expect("Child process already taken");
        let worker_msg_task = self
            .worker_msg_task
            .take()
            .expect("Worker message task already taken");
        let ((), exit_status) = tokio::try_join! {
            async {
                worker_msg_task.await??;
                Ok::<(), IoError>(())
            },
            async {
                let exit_status = child_proc.wait().await?;
                Ok(exit_status)
            }
        }?;
        if !exit_status.success() {
            return Err(IoError::other(format!(
                "Child process exited with status: {exit_status}"
            )));
        }
        Ok(())
    }
}

impl Drop for ChildManager {
    fn drop(&mut self) {
        if let Some(mut child_proc) = self.child_proc.take() {
            drop(tokio::spawn(async move { child_proc.kill().await }));
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let temp_dir = tempfile::tempdir()?;
    eprintln!("Using temp dir: {}", temp_dir.path().display());
    let config = WorkerConfig {
        lock_file_path: temp_dir.path().join("elock_test.lock"),
        hold_ms: TimeRange {
            min: Duration::from_micros(500),
            max: Duration::from_micros(1500),
        },
        use_shared: true,
    };
    let (sender, mut receiver) = mpsc::channel::<(usize, WorkerMessage)>(100);
    let child = {
        let child = ChildManager::spawn_new(0, &config, &sender).await?;
        drop(sender);
        child
    };
    tokio::try_join!(
        async {
            eprintln!("Sleeping for 5 seconds to let worker run...");
            tokio::time::sleep(Duration::from_secs(5)).await;
            eprintln!("5 seconds elapsed, stopping worker...");
            child.send(ManagerMessage::Stop).await?;
            Ok::<(), IoError>(())
        },
        async {
            while let Some((id, msg)) = receiver.recv().await {
                eprintln!("Received message from worker {id}: {msg:?}");
            }
            Ok(())
        }
    )?;
    eprintln!("Waiting for worker to exit...");
    child.join().await?;
    Ok(())
}
