use input::InputState;
use output::OutputState;
use probe::Probe;
use smol::{io::AsyncBufReadExt, stream::StreamExt};

mod formats;
mod input;
mod output;
mod probe;
mod tcp;

pub use formats::{OggVorbisOutputOptions, OutputFormat};
pub use input::{Input, ReaderInput};
pub use output::{Output, VecOutput};

fn split_key_value_line(line: &str) -> Option<(&str, &str)> {
    let eq_index = line.find('=')?;
    let key = &line[..eq_index];
    let value = &line[eq_index + 1..];
    Some((key, value.trim()))
}

pub trait ProgressListener {
    fn on_progress(&mut self, complete_ratio: f32, progress_info: Vec<(String, String)>);
    fn on_done(&mut self) {}
}

pub struct NullProgressListener;

impl ProgressListener for NullProgressListener {
    fn on_progress(&mut self, complete_ratio: f32, _progress_info: Vec<(String, String)>) {
        eprintln!(
            "Progress {:.02}: {:?}",
            (complete_ratio * 100.0),
            _progress_info
        );
    }
}

pub struct FfmpegTool {
    ffmpeg_path: std::path::PathBuf,
    probe: Probe,
}

impl FfmpegTool {
    pub fn from_path(ffmpeg_path: std::path::PathBuf, ffprobe_path: std::path::PathBuf) -> Self {
        FfmpegTool {
            ffmpeg_path,
            probe: Probe::new(ffprobe_path),
        }
    }

    pub async fn convert<I, O>(
        &self,
        input: I,
        output: O,
        output_format: impl Into<formats::OutputFormat>,
        progress: &mut dyn ProgressListener,
    ) -> anyhow::Result<O::OutputType>
    where
        I: Input,
        O: Output,
    {
        let duration = self.probe.read_duration(input.clone()).await?;
        let mut command = smol::process::Command::new(&self.ffmpeg_path);
        let input_state = input.create_state().await?;
        let output_state = output.create_state().await?;
        let output_format = output_format.into();
        let mut child = command
            .arg("-nostdin")
            .arg("-progress")
            .arg("pipe:1")
            .arg("-hide_banner")
            .arg("-i")
            .arg(input_state.url())
            .arg("-f")
            .arg(output_format.format_name())
            .args(output_format.get_options().to_flags(Some("a:0")))
            .arg(output_state.url())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()?;
        let stdout = smol::io::BufReader::new(child.stdout.take().expect("Failed to create pipe."));
        let (status, output, _, _) = futures::join!(
            child.status(),
            output_state.wait(),
            input_state.wait(),
            async move {
                let mut lines = stdout.lines();
                let mut progress_info = Vec::new();
                let mut curr_out_time: u64 = 0;
                while let Some(line) = lines.next().await {
                    let line = line.unwrap();
                    if let Some((key, value)) = split_key_value_line(&line) {
                        if key == "progress" {
                            let complete_ratio = (curr_out_time as f64 / 1_000_000.0) / duration;
                            progress.on_progress(complete_ratio as f32, progress_info);
                            if value.trim() == "end" {
                                progress.on_done();
                            }
                            progress_info = Vec::new();
                        } else {
                            if key == "out_time_ms" {
                                if let Ok(time) = value.parse::<u64>() {
                                    curr_out_time = time;
                                }
                            }
                            let value = value.trim().to_string();
                            progress_info.push((key.to_string(), value));
                        }
                    }
                }
            }
        );
        let status = status?;

        anyhow::ensure!(
            status.success(),
            "ffmpeg process exited with non-zero status: {}",
            status
        );

        output
    }
}
