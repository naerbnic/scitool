use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
    pin::Pin,
};

use scidev::{
    ids::{
        LineId,
        raw::{RawConditionId, RawNounId, RawRoomId, RawSequenceId, RawVerbId},
    },
    resources::{
        ResourceSet, ResourceType,
        types::{
            audio36::{Audio36ResourceBuilder, AudioFormat, VoiceSample, VoiceSampleResources},
            msg::{MessageId, parse_message_resource},
        },
    },
    utils::block::TempStore,
};
use serde::{Deserialize, Serialize};

use crate::{
    book::{Book, builder::BookBuilder, config::BookConfig},
    build::audio::ProgressFactory,
    file::AudioSampleScan,
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

#[derive(Serialize, Deserialize, Debug)]
pub struct Sample {
    pub room: u16,
    pub message_id: MessageId,
    pub clip: AudioClip,
}

#[derive(Serialize, Deserialize, Debug)]
struct SampleSet(Vec<Sample>);

fn legacy_load_dir(path: &Path) -> io::Result<BTreeMap<LineId, AudioClip>> {
    let samples_file = path.join("samples.json");
    let samples_file_contents = std::fs::read(&samples_file)?;
    let mut sample_set: SampleSet =
        serde_json::from_reader(std::io::Cursor::new(samples_file_contents))?;
    let mut clip_map = BTreeMap::new();
    for sample in &mut sample_set.0 {
        let Sample {
            room,
            message_id,
            clip,
        } = sample;
        let line_id = LineId::from_parts(
            RawRoomId::new(*room),
            RawNounId::new(message_id.noun()),
            RawVerbId::new(message_id.verb()),
            RawConditionId::new(message_id.condition()),
            RawSequenceId::new(message_id.sequence()),
        );
        let relative_path = std::mem::take(&mut clip.path);
        clip.path = path.join(relative_path);
        clip_map.insert(line_id, clip.clone());
    }
    Ok(clip_map)
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
                .synthesize(&line.text().to_plain_text())
                .await?;
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
                    )
                    .await?
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

#[derive(Copy, Clone, Debug)]
pub enum ScanType {
    Legacy,
    Scannable,
}

pub fn load_config_from_directory(
    scan_type: ScanType,
    base_dir: impl AsRef<Path>,
) -> anyhow::Result<BTreeMap<LineId, AudioClip>> {
    let base_dir = base_dir.as_ref();
    match scan_type {
        ScanType::Legacy => map_from_legacy_dir(base_dir),
        ScanType::Scannable => map_from_sample_scan(base_dir),
    }
}

fn map_from_legacy_dir(base_dir: &Path) -> anyhow::Result<BTreeMap<LineId, AudioClip>> {
    legacy_load_dir(base_dir).map_err(Into::into)
}

fn map_from_sample_scan(base_dir: &Path) -> anyhow::Result<BTreeMap<LineId, AudioClip>> {
    let scan = AudioSampleScan::read_from_dir(base_dir)?;
    anyhow::ensure!(
        !scan.has_duplicates(),
        "Duplicate files found in scan directory",
    );
    let mut clip_map = BTreeMap::new();
    for (line_id, entry) in scan.get_valid_entries() {
        let clip = AudioClip {
            #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            start_us: Some((entry.start() * 1_000_000.0) as u64),
            #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            end_us: entry.end().map(|end| (end * 1_000_000.0) as u64),
            path: scan.base_path().join(entry.path()),
        };
        clip_map.insert(line_id, clip);
    }
    Ok(clip_map)
}

pub fn load_book_from_resources(config: &BookConfig, game_path: &Path) -> anyhow::Result<Book> {
    let resource_set = ResourceSet::from_root_dir(game_path)?;
    let mut builder = BookBuilder::new(config.clone())?;

    // Extra testing for building a conversation.

    for res in resource_set.resources_of_type(ResourceType::Message) {
        let msg_resources = parse_message_resource(&res.data().open_mem(..)?)?;
        for (msg_id, record) in msg_resources.messages() {
            builder.add_message(res.id().resource_num(), msg_id, record)?;
        }
    }
    let book = builder.build()?;
    Ok(book)
}
