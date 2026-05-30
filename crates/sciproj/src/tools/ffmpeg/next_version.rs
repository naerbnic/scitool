//! Experiment with a new API for ffmpeg wrapping.
#![allow(dead_code)]

use std::{
    path::Path,
    pin::Pin,
    task::{Context, Poll},
};

use pin_project::pin_project;
use tokio::{io::ReadBuf, process::Child, task::JoinSet};

use crate::{imp::futures::prelude::*, tools::ffmpeg::formats};

const FFMPEG_INIT_FLAGS: &[&str] = &["-hide_banner"];
const FFMPEG_INPUT_FLAGS: &[&str] = &["-i", "pipe:0"];

async fn start_ffmpeg(
    ffmpeg_path: &Path,
    output_format: impl Into<formats::OutputFormat>,
    start_ns: Option<u64>,
    end_ns: Option<u64>,
) -> std::io::Result<Child> {
    let output_format = output_format.into();
    let mut opts = output_format.get_options();
    if let Some(start_ns) = start_ns {
        opts = opts.add_flag("ss", start_ns.to_string());
    }
    if let Some(end_ns) = end_ns {
        opts = opts.add_flag("to", end_ns.to_string());
    }
    let child = tokio::process::Command::new(ffmpeg_path)
        .args(FFMPEG_INIT_FLAGS)
        // The input comes from stdin
        .args(FFMPEG_INPUT_FLAGS)
        .arg("-f")
        .arg(output_format.format_name())
        .args(opts.to_flags(Some("a:0")))
        // The output comes from stdout
        .arg("pipe:1")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    Ok(child)
}

// Placeholder trait for starting an ffmpeg process

trait ProcessCreator {
    async fn create_process(&self) -> tokio::process::Child;
}

#[pin_project]
pub struct ConverterReader {
    #[pin]
    reader: tokio::io::ReadHalf<tokio::io::SimplexStream>,
    join_set: JoinSet<std::io::Result<()>>,
}

impl ConverterReader {
    pub async fn new<R>(
        input: R,
        ffmpeg_path: impl AsRef<Path>,
        output_format: impl Into<formats::OutputFormat>,
        start_ns: Option<u64>,
        end_ns: Option<u64>,
    ) -> std::io::Result<Self>
    where
        R: AsyncRead + Send + 'static,
    {
        // Start the process. It will have stdin and stdout handles
        // available.
        let mut child = start_ffmpeg(ffmpeg_path.as_ref(), output_format, start_ns, end_ns).await?;
        let mut proc_in = child.stdin.take().unwrap();
        let mut proc_out = child.stdout.take().unwrap();
        assert!(child.stderr.is_none());

        // Create a JoinSet to own all of our helper spawned tasks.
        let mut tasks = JoinSet::new();

        // The first task: copy from the input reader to the process stdin.
        tasks.spawn(async move {
            let mut input = std::pin::pin!(input);
            tokio::io::copy(&mut input, &mut proc_in)
                .await
                .or_else(|e| {
                    // We ignore BrokenPipe errors, as those are generally caused
                    // by stdin closing early, which will likely be raised from
                    // another source. Other errors are unexpected.
                    if e.kind() == std::io::ErrorKind::BrokenPipe {
                        Ok(0)
                    } else {
                        Err(e)
                    }
                })?;
            let _unused = proc_in.shutdown().await;
            Ok(())
        });

        // The second task: copy from stdout to the output simplex, and take
        // ownership with the child's lifecycle.

        let (reader, mut writer) = tokio::io::simplex(8192);

        tasks.spawn(async move {
            tokio::io::copy(&mut proc_out, &mut writer).await?;
            // Before we shutdown the writer, we want to be able to wait on the
            // process.
            let exit_status = child.wait().await?;

            if !exit_status.success() {
                return Err(std::io::Error::other(format!(
                    "child process exited with exit code {exit_status}"
                )));
            }

            // Q: What do we do with the exit status here?
            // Regardless, the child has completed, and we can finish our
            // stream.
            writer.shutdown().await?;

            Ok::<(), std::io::Error>(())
        });

        Ok(Self {
            reader,
            join_set: tasks,
        })
    }
}

impl AsyncRead for ConverterReader {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let self_mut = self.project();
        while let Poll::Ready(result) = self_mut.join_set.poll_join_next(cx) {
            let Some(task_result) = result else {
                // If we get None here, that means that there are no more
                // tasks in the join set. Escape the while loop.
                break;
            };
            match task_result {
                Ok(Ok(())) => {
                    // Task completed validly. Move on to the next one.
                }
                Ok(Err(io_err)) => {
                    // One of our tasks failed. We surface the error as a
                    // result of the read.
                    return Poll::Ready(Err(io_err));
                }
                Err(join_error) => {
                    // We don't expect to be cancelled, as that should only
                    // happen when this object gets dropped, and we
                    // wouldn't be polled if we did.
                    assert!(!join_error.is_cancelled());
                    match join_error.try_into_panic() {
                        Ok(payload) => {
                            // We want to propagate the panic, so the
                            // error isn't silently ignored.
                            std::panic::resume_unwind(payload);
                        }
                        Err(_) => {
                            // We have handled the two known reasons we get a
                            // join error. Give up for now.
                            unreachable!()
                        }
                    }
                }
            }
        }
        self_mut.reader.poll_read(cx, buf)
    }
}
