use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

use futures::{prelude::*, stream::FuturesUnordered};
use itertools::Itertools;
use scidev::utils::block::temp_store::TempStore;
use scidev::{
    common::{LineId, RawConditionId, RawNounId, RawRoomId, RawSequenceId, RawVerbId},
    resources::types::{
        audio36::{Audio36ResourceBuilder, AudioFormat, VoiceSample, VoiceSampleResources},
        msg::MessageId,
    },
};
use serde::{Deserialize, Serialize};

use crate::{
    file::AudioSampleScan,
    tools::ffmpeg::{self, FfmpegTool, OggVorbisOutputOptions},
};

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
            let result = ffmpeg
                .convert(
                    base_path.join(&clip_path),
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
        let mut temp_store = TempStore::create()?;
        while let Some(result) = conversion_stream.next().await {
            let sample = result?;
            // Only VecDeque implements Buffer.
            let sample_source = temp_store.store_bytes(&sample.data[..]).await?;
            let voice_sample = VoiceSample::new(AudioFormat::Ogg, sample_source);
            builder.add_entry(sample.room, sample.message_id, &voice_sample)?;
        }
        Ok(builder.build()?)
    }
}

pub struct SampleDir {
    base_path: PathBuf,
    samples: SampleSet,
}

impl SampleDir {
    pub async fn load_dir(path: &Path) -> anyhow::Result<Self> {
        let samples_file = path.join("samples.json");
        let samples_file_contents = tokio::fs::read(&samples_file).await?;
        let sample_set: SampleSet =
            serde_json::from_reader(std::io::Cursor::new(samples_file_contents))?;
        Ok(Self {
            base_path: path.to_path_buf(),
            samples: sample_set,
        })
    }

    pub fn from_sample_scan(scan: &AudioSampleScan) -> anyhow::Result<Self> {
        anyhow::ensure!(!scan.has_duplicates(), "Input scan must have no duplicates");
        let mut samples = Vec::new();
        for (line_id, entry) in scan.get_valid_entries() {
            let msg_id = MessageId::new(
                line_id.noun_num(),
                line_id.verb_num(),
                line_id.condition_num(),
                line_id.sequence_num(),
            );

            let clip = AudioClip {
                #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                start_us: Some((entry.start() * 1_000_000.0) as u64),
                #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                end_us: entry.end().map(|end| (end * 1_000_000.0) as u64),
                path: entry.path().to_path_buf(),
            };

            let sample = Sample {
                room: line_id.room_num(),
                message_id: msg_id,
                clip,
            };
            samples.push(sample);
        }
        Ok(Self {
            base_path: scan.base_path().to_path_buf(),
            samples: SampleSet(samples),
        })
    }

    pub async fn save_to_scannable_dir(&self, path: &Path) -> anyhow::Result<()> {
        // Check that all files contain a single message ID.
        let path_list = self
            .samples
            .0
            .iter()
            .map(|sample| {
                let clip = &sample.clip;
                anyhow::ensure!(clip.start_us.is_none_or(|off| off == 0));
                anyhow::ensure!(clip.end_us.is_none());
                Ok(&sample.clip.path)
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let multi_path_counts = path_list
            .into_iter()
            .map(|path| (path, 1))
            .into_grouping_map()
            .sum()
            .into_iter()
            .filter(|(_, count)| *count > 1)
            .collect::<Vec<_>>();
        if !multi_path_counts.is_empty() {
            return Err(anyhow::anyhow!(
                "The following paths have multiple message IDs: {:?}",
                multi_path_counts
            ));
        }

        // Go through all of the clips, and copy the files with the line ID
        // as the file name.
        let mut copy_operations = self
            .samples
            .0
            .iter()
            .map(async |sample| {
                let line_id = LineId::from_parts(
                    RawRoomId::new(sample.room),
                    RawNounId::new(sample.message_id.noun()),
                    RawVerbId::new(sample.message_id.verb()),
                    RawConditionId::new(sample.message_id.condition()),
                    RawSequenceId::new(sample.message_id.sequence()),
                );
                let clip = &sample.clip;
                let current_path = &clip.path;
                let mut file_name: OsString = line_id.to_string().into();
                if let Some(ext) = current_path.extension() {
                    file_name.push(".");
                    file_name.push(ext);
                }
                let mut new_path = current_path.clone();
                new_path.set_file_name(file_name);
                let source_path = self.base_path.join(current_path);
                let target_path = path.join(new_path);
                // Create the target directory if it doesn't exist.
                let target_dir = target_path.parent().unwrap();
                tokio::fs::create_dir_all(target_dir).await?;
                // Copy the file to the new location.
                tokio::fs::create_dir_all(target_dir).await?;
                tokio::fs::copy(source_path, target_path).await?;
                Ok::<_, anyhow::Error>(())
            })
            .collect::<FuturesUnordered<_>>();

        while let Some(result) = copy_operations.next().await {
            result?;
        }
        Ok(())
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
