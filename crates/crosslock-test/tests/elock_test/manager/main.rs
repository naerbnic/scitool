use std::{
    io::{Error as IoError, Result as IoResult},
    process::Stdio,
    time::Duration,
};

use futures::{SinkExt, StreamExt, TryStreamExt, stream::FuturesUnordered};
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

const NUM_WORKERS: usize = 20;

struct ChildManager {
    child_proc: Option<tokio::process::Child>,
    msg_output: AsyncMutex<Option<BoxedMessageSink<Value>>>,
    worker_msg_task: Option<JoinHandle<IoResult<()>>>,
}

impl ChildManager {
    async fn spawn_new(id: usize, sender: mpsc::Sender<(usize, WorkerMessage)>) -> IoResult<Self> {
        let mut child_proc = tokio::process::Command::new(env!("CARGO_BIN_EXE_elock_test_worker"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let child_stdin = child_proc.stdin.take().expect("Failed to open stdin");
        let child_stdout = child_proc.stdout.take().expect("Failed to open stdout");
        let msg_output = create_message_sink(child_stdin);
        let msg_input = create_message_stream(child_stdout);
        let worker_msg_task = tokio::spawn({
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

struct ChildSet {
    children: Vec<ChildManager>,
}

impl ChildSet {
    async fn new(
        num_children: usize,
        sender: mpsc::Sender<(usize, WorkerMessage)>,
    ) -> IoResult<Self> {
        let children = (0..num_children)
            .map(|id| {
                let sender = sender.clone();
                async move { ChildManager::spawn_new(id, sender.clone()).await }
            })
            .collect::<FuturesUnordered<_>>()
            .try_collect()
            .await?;
        Ok(Self { children })
    }

    async fn apply_to_all<'a, F, Fut>(&'a self, f: F) -> IoResult<()>
    where
        F: Fn(&'a ChildManager) -> Fut,
        Fut: Future<Output = IoResult<()>> + Send + 'a,
    {
        let () = self
            .children
            .iter()
            .map(f)
            .collect::<FuturesUnordered<_>>()
            .try_collect()
            .await?;
        Ok(())
    }

    async fn start_all(&self, config: &WorkerConfig) -> IoResult<()> {
        self.apply_to_all(
            |child| async move { child.send(ManagerMessage::Start(config.clone())).await },
        )
        .await
    }

    async fn stop_all(&self) -> IoResult<()> {
        self.apply_to_all(|child| async move { child.send(ManagerMessage::Stop).await })
            .await
    }

    async fn join_all(self) -> IoResult<()> {
        let () = self
            .children
            .into_iter()
            .map(ChildManager::join)
            .collect::<FuturesUnordered<_>>()
            .try_collect()
            .await?;
        Ok(())
    }
}

struct WorkerStats {
    num_locks_acquired: usize,
    total_lock_time: Duration,
    total_unlock_time: Duration,
    prev_lock_start: Option<std::time::SystemTime>,
    time_ranges: Vec<(std::time::SystemTime, std::time::SystemTime)>,
}

impl WorkerStats {
    pub(crate) fn new() -> Self {
        Self {
            num_locks_acquired: 0,
            total_lock_time: Duration::from_millis(0),
            total_unlock_time: Duration::from_millis(0),
            prev_lock_start: None,
            time_ranges: Vec::new(),
        }
    }

    pub(crate) fn handle_worker_message(&mut self, msg: &WorkerMessage) {
        match msg {
            WorkerMessage::LockAcquired(acquired) => {
                self.num_locks_acquired += 1;
                self.total_lock_time += acquired.lock_time;
                assert!(self.prev_lock_start.is_none(), "Had a double lock!");
                self.prev_lock_start = Some(acquired.lock_start);
            }
            WorkerMessage::LockReleased(released) => {
                self.total_unlock_time += released.unlock_time;
                if let Some(lock_start) = self.prev_lock_start.take() {
                    assert!(
                        lock_start <= released.lock_end,
                        "Lock end time is before lock start time!"
                    );
                    self.time_ranges.push((lock_start, released.lock_end));
                } else {
                    panic!("Had a unlock without a matching lock!");
                }
            }
        }
    }
}

impl WorkerSetStats {
    pub(crate) fn new(num_workers: usize) -> Self {
        Self {
            workers: (0..num_workers).map(|_| WorkerStats::new()).collect(),
        }
    }

    pub(crate) fn handle_worker_message(&mut self, id: usize, msg: &WorkerMessage) {
        self.workers[id].handle_worker_message(msg);
    }

    pub(crate) fn print_summary(&self) {
        let num_locks_acquired: u32 = u32::try_from(
            self.workers
                .iter()
                .map(|w| w.num_locks_acquired)
                .sum::<usize>(),
        )
        .unwrap();
        let max_locks_acquired = self
            .workers
            .iter()
            .map(|w| w.num_locks_acquired)
            .max()
            .unwrap_or(0);
        eprintln!("Max locks acquired by a single worker: {max_locks_acquired}");
        let min_locks_acquired = self
            .workers
            .iter()
            .map(|w| w.num_locks_acquired)
            .min()
            .unwrap_or(0);
        eprintln!("Min locks acquired by a single worker: {min_locks_acquired}");
        let (max_lock_time, id) = self
            .workers
            .iter()
            .enumerate()
            .map(|(id, w)| (w.total_lock_time, id))
            .max()
            .unwrap();
        eprintln!("Worker {id} had the longest total lock wait time: {max_lock_time:?}");
        let (min_lock_time, id) = self
            .workers
            .iter()
            .enumerate()
            .map(|(id, w)| (w.total_lock_time, id))
            .min()
            .unwrap();
        eprintln!("Worker {id} had the shortest total lock wait time: {min_lock_time:?}");
        let total_lock_time: Duration = self.workers.iter().map(|w| w.total_lock_time).sum();
        let total_unlock_time: Duration = self.workers.iter().map(|w| w.total_unlock_time).sum();
        eprintln!("Total locks acquired: {num_locks_acquired}");
        if num_locks_acquired > 0 {
            eprintln!(
                "Average lock time: {:.2?}",
                total_lock_time / num_locks_acquired
            );
            eprintln!(
                "Average unlock time: {:.2?}",
                total_unlock_time / num_locks_acquired
            );
        }
    }

    pub(crate) fn validate_time_ranges(&self) {
        let mut all_ranges = self
            .workers
            .iter()
            .flat_map(|w| w.time_ranges.iter())
            .collect::<Vec<_>>();
        all_ranges.sort_by_key(|(start, _end)| *start);
        for pair in all_ranges.windows(2) {
            if let [(start1, end1), (start2, end2)] = pair {
                assert!(
                    end1 <= start2,
                    "Detected overlapping lock time ranges: ({start1:?}, {end1:?}) and ({start2:?}, {end2:?})"
                );
            }
        }
        eprintln!("All lock time ranges are valid and non-overlapping.");
    }
}

struct WorkerSetStats {
    workers: Vec<WorkerStats>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let temp_dir = tempfile::tempdir()?;
    eprintln!("Using temp dir: {}", temp_dir.path().display());
    let lock_path = temp_dir.path().join("elock_test.lock");
    let config = WorkerConfig {
        lock_file_path: lock_path.clone(),
        hold_ms: TimeRange {
            min: Duration::from_micros(500),
            max: Duration::from_micros(1500),
        },
        use_shared: false,
    };
    let (sender, mut receiver) = mpsc::channel::<(usize, WorkerMessage)>(100);
    let child_set = ChildSet::new(NUM_WORKERS, sender).await?;

    child_set.start_all(&config).await?;

    let mut stats = WorkerSetStats::new(NUM_WORKERS);

    tokio::try_join!(
        async {
            const TEST_DURATION: Duration = Duration::from_secs(5);
            eprintln!("Sleeping for {TEST_DURATION:?} to let workers run...");
            tokio::time::sleep(TEST_DURATION).await;
            eprintln!("{TEST_DURATION:?} elapsed, stopping workers...");
            child_set.stop_all().await?;
            Ok::<(), IoError>(())
        },
        async {
            while let Some((id, msg)) = receiver.recv().await {
                stats.handle_worker_message(id, &msg);
            }
            Ok(())
        }
    )?;
    eprintln!("Waiting for workers to exit...");
    child_set.join_all().await?;

    stats.print_summary();
    stats.validate_time_ranges();
    if std::fs::exists(&lock_path)? {
        eprintln!("Lock file still exists: {}", lock_path.display());
    } else {
        eprintln!("Lock file was deleted correctly.");
    }
    Ok(())
}
