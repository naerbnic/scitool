use std::path::PathBuf;

use futures::StreamExt;
use sci_resources::types::{
    audio36::{Audio36ResourceBuilder, AudioFormat, VoiceSample, VoiceSampleResources},
    msg::MessageId,
};
use sci_utils::block::temp_store::TempStore;
use serde::{Deserialize, Serialize};

use crate::tools::ffmpeg::{self, FfmpegTool};

#[derive(Serialize, Deserialize, Debug)]
pub struct AudioClip {
    pub start_us: u64,
    pub end_us: u64,
    pub path: PathBuf,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Sample {
    pub room: u16,
    pub message_id: MessageId,
    pub clip: AudioClip,
}

pub struct SampleSet(Vec<Sample>);

impl SampleSet {
    pub async fn to_audio_resources(
        &self,
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
            let sample_file = smol::fs::File::open(&sample.clip.path).await?;
            let result = ffmpeg
                .convert(
                    ffmpeg::ReaderInput::new(sample_file),
                    ffmpeg::VecOutput,
                    ffmpeg::OutputFormat::Ogg(Default::default()),
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
