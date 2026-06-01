mod formats;
mod next_version;

pub use formats::{OggVorbisOutputOptions, OutputFormat};
pub use next_version::ConverterReader;

use crate::{imp::futures::prelude::*, tools::Tool};

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

pub struct FfmpegTool {
    tool: Tool,
}

impl FfmpegTool {
    #[must_use]
    pub fn from_tool(tool: Tool) -> Self {
        FfmpegTool { tool }
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
            &self.tool,
            output_format,
            start_ns,
            end_ns,
        )?)
    }
}
