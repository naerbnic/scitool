mod formats;
mod next_version;

pub use formats::{OggVorbisOutputOptions, OutputFormat};
pub use next_version::ConverterReader;

use crate::imp::futures::prelude::*;

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
    ffmpeg_path: std::path::PathBuf,
}

impl FfmpegTool {
    #[must_use]
    pub fn from_path(ffmpeg_path: std::path::PathBuf) -> Self {
        FfmpegTool { ffmpeg_path }
    }

    pub async fn create_convert_reader<R>(
        &self,
        reader: R,
        output_format: impl Into<formats::OutputFormat>,
    ) -> anyhow::Result<ConverterReader>
    where
        R: AsyncRead + Send + 'static,
    {
        Ok(ConverterReader::new(reader, &self.ffmpeg_path, output_format).await?)
    }
}
