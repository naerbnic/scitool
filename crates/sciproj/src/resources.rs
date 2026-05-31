use std::{collections::BTreeMap, path::PathBuf, pin::Pin};

use scidev::{
    ids::LineId,
    resources::types::{
        audio36::{Audio36ResourceBuilder, AudioFormat, VoiceSample, VoiceSampleResources},
        msg::MessageId,
    },
    utils::block::TempStore,
};
use serde::{Deserialize, Serialize};

use crate::{
    book::Book,
    build::audio::ProgressFactory,
    imp::futures::{self, prelude::*},
    tools::{
        espeak::EspeakTool,
        ffmpeg::{self, FfmpegTool, OggVorbisOutputOptions},
    },
};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AudioClip {
    pub start_us: Option<u64>,
    pub end_us: Option<u64>,
    pub path: PathBuf,
}

fn to_dyn_async_read<R: AsyncRead + Send + 'static>(data: R) -> Pin<Box<dyn AsyncRead + Send>> {
    Box::pin(data)
}
struct ProcessedSample {
    line_id: LineId,
    data: Vec<u8>,
}

struct PreparedInput {
    line_id: LineId,
    data: Pin<Box<dyn AsyncRead + Send>>,
    start_ns: Option<u64>,
    end_ns: Option<u64>,
}

pub async fn generate_sample_resources(
    progress: ProgressFactory,
    book: &Book,
    line_mapping: &BTreeMap<LineId, AudioClip>,
    ffmpeg: &FfmpegTool,
    espeak: Option<&EspeakTool>,
) -> anyhow::Result<VoiceSampleResources> {
    // Split the results into those that have a clip, and the others which are
    // not built, or synthesized.

    let mut clipped_lines = Vec::new();
    let mut unclipped_lines = Vec::new();
    for line in book.lines() {
        let line_id = line.id();
        if let Some(clip) = line_mapping.get(&line_id) {
            clipped_lines.push((line, clip));
        } else if espeak.is_some() {
            unclipped_lines.push(line);
        }
    }

    let operation_count = clipped_lines.len() + unclipped_lines.len();

    let clipped_processed_samples = clipped_lines.into_iter().map(|(line, clip)| {
        async move {
            Ok::<_, anyhow::Error>(PreparedInput {
                line_id: line.id(),
                data: to_dyn_async_read(tokio::fs::File::open(&clip.path).await?),
                start_ns: clip.start_us,
                end_ns: clip.end_us,
            })
        }
        .boxed()
    });

    let unclipped_processed_samples = unclipped_lines.into_iter().map(|line| {
        async move {
            let synthesized = espeak
                .as_ref()
                .unwrap()
                .synthesize(&line.text().to_plain_text())?;
            Ok::<_, anyhow::Error>(PreparedInput {
                line_id: line.id(),
                data: to_dyn_async_read(synthesized),
                start_ns: None,
                end_ns: None,
            })
        }
        .boxed()
    });

    let generate_clip_progress =
        progress.make_count_bar(operation_count.try_into().unwrap(), "Generating clips");

    let processed_samples: Vec<ProcessedSample> =
        futures::stream::iter(clipped_processed_samples.chain(unclipped_processed_samples))
            .map(async |source_data_fut| {
                let source_data = source_data_fut.await?;
                let mut data = Vec::new();
                ffmpeg
                    .create_convert_reader(
                        source_data.data,
                        ffmpeg::OutputFormat::Ogg(OggVorbisOutputOptions::new(4, Some(22050))),
                        source_data.start_ns,
                        source_data.end_ns,
                    )?
                    .read_to_end(&mut data)
                    .await?;
                generate_clip_progress.inc(1);
                generate_clip_progress
                    .set_message(format!("Generated clip for {}", source_data.line_id));
                Ok::<_, anyhow::Error>(ProcessedSample {
                    line_id: source_data.line_id,
                    data,
                })
            })
            .buffer_unordered(10)
            .try_collect()
            .await?;

    let mut builder = Audio36ResourceBuilder::new();
    tokio::task::spawn_blocking({
        let progress =
            progress.make_count_bar(operation_count.try_into().unwrap(), "Building resources");
        move || {
            let mut temp_store = TempStore::create()?;
            for sample in processed_samples {
                let line_id = sample.line_id;
                let message_id = MessageId::new(
                    line_id.noun_num(),
                    line_id.verb_num(),
                    line_id.condition_num(),
                    line_id.sequence_num(),
                );
                let sample_source = temp_store.store_bytes(&sample.data[..])?;
                let voice_sample = VoiceSample::new(AudioFormat::Ogg, sample_source);
                builder.add_entry(line_id.room_num(), message_id, &voice_sample)?;
                progress.inc(1);
                progress.set_message(format!(
                    "Added {line_id} to resource #{}",
                    line_id.room_num()
                ));
            }
            Ok(builder.build()?)
        }
    })
    .await?
}
