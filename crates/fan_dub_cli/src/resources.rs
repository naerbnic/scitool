use std::path::{Path, PathBuf};

use futures::StreamExt;
use sci_resources::types::{
    audio36::{Audio36ResourceBuilder, AudioFormat, VoiceSample, VoiceSampleResources},
    msg::MessageId,
};
use sci_utils::block::temp_store::TempStore;
use serde::{Deserialize, Serialize};

use crate::tools::ffmpeg::{self, FfmpegTool, OggVorbisOutputOptions};

fn normalize_path(path: &Path) -> PathBuf {
    let mut result_buf = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::Prefix(prefix) => {
                // This should only happen with Windows paths, and will happen
                // when a path starts with a drive letter.
                result_buf.push(prefix.as_os_str());
            }
            std::path::Component::RootDir => {
                result_buf.push(std::path::MAIN_SEPARATOR_STR);
            }
            std::path::Component::CurDir => {
                // Should skip this component.
            }
            std::path::Component::ParentDir => {
                // There is the possibility that foo/../bar isn't the same as
                // bar, depending on the interpretation of symbolic links.
                //
                // To prevent this from being a problem, users of this should
                // always use the normalized path instead of the original path,
                // as they may be subtly semantically different.
                if !result_buf.pop() {
                    // We're at the top-level of the path, so, just append the
                    // parent directory.
                    result_buf.push("..");
                }
            }
            std::path::Component::Normal(elem) => {
                result_buf.push(elem);
            }
        }
    }
    result_buf
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AudioClip {
    pub start_us: Option<u64>,
    pub end_us: Option<u64>,
    pub path: PathBuf,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Sample {
    pub room: u16,
    pub message_id: MessageId,
    pub clip: AudioClip,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SampleSet(Vec<Sample>);

impl SampleSet {
    pub async fn to_audio_resources(
        &self,
        base_path: &Path,
        ffmpeg: &FfmpegTool,
        num_concurrent: usize,
    ) -> anyhow::Result<VoiceSampleResources> {
        struct ProcessedSample {
            room: u16,
            message_id: MessageId,
            data: Vec<u8>,
        }
        let mut builder = Audio36ResourceBuilder::new();
        let conversion_ops = self.0.iter().map(|sample| async {
            let clip_path = normalize_path(&sample.clip.path);
            anyhow::ensure!(
                clip_path.is_relative(),
                "A path for an audio clip must be relative to the root directory."
            );
            let sample_file = smol::fs::File::open(base_path.join(clip_path)).await?;
            let result = ffmpeg
                .convert(
                    ffmpeg::ReaderInput::new(sample_file),
                    ffmpeg::VecOutput,
                    ffmpeg::OutputFormat::Ogg(OggVorbisOutputOptions::new(4, Some(22050))),
                    &mut ffmpeg::NullProgressListener,
                )
                .await?;
            Ok::<_, anyhow::Error>(ProcessedSample {
                room: sample.room,
                message_id: sample.message_id,
                data: result,
            })
        });

        let mut conversion_stream =
            futures::stream::iter(conversion_ops).buffer_unordered(num_concurrent);
        let mut temp_store = TempStore::new()?;
        while let Some(result) = conversion_stream.next().await {
            let sample = result?;
            // Only VecDeque implements Buffer.
            let sample_source = temp_store.store_bytes(&sample.data[..]).await?;
            let voice_sample = VoiceSample::new(AudioFormat::Ogg, sample_source);
            builder.add_entry(sample.room, sample.message_id, voice_sample)?;
        }
        builder.build()
    }
}

pub struct SampleDir {
    base_path: PathBuf,
    samples: SampleSet,
}

impl SampleDir {
    pub async fn load_dir(path: &Path) -> anyhow::Result<Self> {
        let samples_file = path.join("samples.json");
        let samples_file_contents = smol::fs::read(&samples_file).await?;
        let sample_set: SampleSet =
            serde_json::from_reader(std::io::Cursor::new(samples_file_contents))?;
        Ok(Self {
            base_path: path.to_path_buf(),
            samples: sample_set,
        })
    }

    pub async fn to_audio_resources(
        &self,
        ffmpeg: &FfmpegTool,
        num_concurrent: usize,
    ) -> anyhow::Result<VoiceSampleResources> {
        self.samples
            .to_audio_resources(&self.base_path, ffmpeg, num_concurrent)
            .await
    }
}
