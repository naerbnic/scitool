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

pub async fn generate_sample_resources(
    book: &Book,
    line_mapping: &BTreeMap<LineId, AudioClip>,
    ffmpeg: &FfmpegTool,
    espeak: Option<&EspeakTool>,
) -> anyhow::Result<VoiceSampleResources> {
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
    let mut builder = Audio36ResourceBuilder::new();
    let processed_samples: Vec<ProcessedSample> = futures::stream::iter(book.lines())
        .map(Ok::<_, anyhow::Error>)
        .try_filter_map(async |line| {
            let line_id = line.id();
            let (source_data, start_ns, end_ns) = if let Some(clip) = line_mapping.get(&line_id) {
                (
                    to_dyn_async_read(tokio::fs::File::open(&clip.path).await?),
                    clip.start_us,
                    clip.end_us,
                )
            } else if let Some(espeak) = espeak {
                let synthesized = espeak.synthesize(&line.text().to_plain_text()).await?;
                (to_dyn_async_read(synthesized), None, None)
            } else {
                return Ok(None);
            };

            Ok(Some(PreparedInput {
                line_id,
                data: source_data,
                start_ns,
                end_ns,
            }))
        })
        .map(async |source_data| {
            let source_data = source_data?;
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
            Ok::<_, anyhow::Error>(ProcessedSample {
                line_id: source_data.line_id,
                data,
            })
        })
        .buffer_unordered(10)
        .try_collect()
        .await?;

    tokio::task::spawn_blocking(|| {
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
        }
        Ok(builder.build()?)
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
