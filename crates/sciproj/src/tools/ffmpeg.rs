mod formats;
mod next_version;

use std::{pin::Pin, process::Output};

pub use formats::{OggVorbisOutputOptions, OutputFormat};
pub use next_version::ConverterReader;
use tokio::process::Command;

use crate::{
    imp::futures::prelude::*,
    path::abspath::AbsPathBuf,
    tools::{TestableTool, location::ToolLocation},
};

pub trait ProgressListener {
    fn on_progress(&mut self, complete_ratio: f32, progress_info: Vec<(String, String)>);
    fn on_done(&mut self) {}
}

pub struct NullProgressListener;

impl ProgressListener for NullProgressListener {
    fn on_progress(&mut self, complete_ratio: f32, progress_info: Vec<(String, String)>) {
        eprintln!(
            "Progress {:.02}: {:?}",
            (complete_ratio * 100.0),
            progress_info
        );
    }
}

pub type BoxFfmpeg = Box<dyn Ffmpeg>;

pub trait Ffmpeg: TestableTool {
    fn convert(
        &self,
        reader: Pin<Box<dyn AsyncRead + Send>>,
        output_format: formats::OutputFormat,
        start_ns: Option<u64>,
        end_ns: Option<u64>,
    ) -> anyhow::Result<Pin<Box<dyn AsyncRead + Send>>>;
}

pub fn from_tool_location(location: &ToolLocation) -> anyhow::Result<BoxFfmpeg> {
    Ok(Box::new(FfmpegTool::from_bin_path(location.bin_path())))
}

#[derive(Debug)]
pub struct FfmpegTool {
    bin_path: AbsPathBuf,
}

impl FfmpegTool {
    #[must_use]
    pub fn from_bin_path(bin_path: AbsPathBuf) -> Self {
        FfmpegTool { bin_path }
    }

    pub fn create_convert_reader<R>(
        &self,
        reader: R,
        output_format: impl Into<formats::OutputFormat>,
        start_ns: Option<u64>,
        end_ns: Option<u64>,
    ) -> anyhow::Result<ConverterReader>
    where
        R: AsyncRead + Send + 'static,
    {
        Ok(ConverterReader::new(
            reader,
            &self.bin_path,
            output_format,
            start_ns,
            end_ns,
        )?)
    }

    async fn test_binary_impl(&self) -> anyhow::Result<Output> {
        let mut cmd = Command::new(&self.bin_path);
        Ok(cmd.arg("-version").output().await?)
    }
}

impl TestableTool for FfmpegTool {
    fn test_binary<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Output>> + Send + 'a>> {
        Box::pin(self.test_binary_impl())
    }
}

impl Ffmpeg for FfmpegTool {
    fn convert(
        &self,
        reader: Pin<Box<dyn AsyncRead + Send>>,
        output_format: formats::OutputFormat,
        start_ns: Option<u64>,
        end_ns: Option<u64>,
    ) -> anyhow::Result<Pin<Box<dyn AsyncRead + Send>>> {
        let converted = self.create_convert_reader(reader, output_format, start_ns, end_ns)?;
        Ok(Box::pin(converted))
    }
}
