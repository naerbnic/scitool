use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use futures::prelude::*;
use scidev::{
    ids::LineId,
    resources::types::{
        audio36::{Audio36ResourceBuilder, AudioFormat, VoiceSample, VoiceSampleResources},
        msg::MessageId,
    },
    utils::block::TempStore,
};
use sciproj::{
    file::AudioSampleScan,
    path::LookupPath,
    resources::{AudioClip, legacy_load_dir},
    tools::ffmpeg::{self, FfmpegTool, OggVorbisOutputOptions},
};
use tokio::io::AsyncReadExt;

#[derive(Copy, Clone, Debug)]
pub(crate) enum ScanType {
    Legacy,
    Scannable,
}

async fn generate_sample_resources(
    line_mapping: &BTreeMap<LineId, AudioClip>,
    ffmpeg: &FfmpegTool,
) -> anyhow::Result<VoiceSampleResources> {
    struct ProcessedSample {
        room: u16,
        message_id: MessageId,
        data: Vec<u8>,
    }
    let mut builder = Audio36ResourceBuilder::new();
    let processed_samples: Vec<ProcessedSample> = futures::stream::iter(line_mapping)
        .map(async |(line_id, clip)| {
            let input_file = tokio::fs::File::open(&clip.path).await?;
            let mut data = Vec::new();
            ffmpeg
                .create_convert_reader(
                    input_file,
                    ffmpeg::OutputFormat::Ogg(OggVorbisOutputOptions::new(4, Some(22050))),
                )
                .await?
                .read_to_end(&mut data)
                .await?;
            Ok::<_, anyhow::Error>(ProcessedSample {
                room: line_id.room_num(),
                message_id: MessageId::new(
                    line_id.noun_num(),
                    line_id.verb_num(),
                    line_id.condition_num(),
                    line_id.sequence_num(),
                ),
                data,
            })
        })
        .map(Ok)
        .try_buffer_unordered(10)
        .try_collect()
        .await?;

    tokio::task::spawn_blocking(|| {
        let mut temp_store = TempStore::create()?;
        for sample in processed_samples {
            let sample_source = temp_store.store_bytes(&sample.data[..])?;
            let voice_sample = VoiceSample::new(AudioFormat::Ogg, sample_source);
            builder.add_entry(sample.room, sample.message_id, &voice_sample)?;
        }
        Ok(builder.build()?)
    })
    .await?
}

fn with_local_runtime<F, T, E>(fut: F) -> Result<T, E>
where
    F: Future<Output = Result<T, E>>,
    E: From<std::io::Error>,
{
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(fut)
}

async fn try_spawn_blocking<F, T, E>(op: F) -> Result<T, E>
where
    F: FnOnce() -> Result<T, E> + Send + 'static,
    T: Send + 'static,
    E: From<tokio::task::JoinError> + Send + 'static,
{
    tokio::task::spawn_blocking(op).await?
}

pub(crate) async fn compile_audio_base(
    line_mapping: &BTreeMap<LineId, AudioClip>,
    output_dir: &Path,
) -> anyhow::Result<()> {
    let system_path = LookupPath::from_env();
    log::info!("System PATH: {:?}", system_path.find_binary("ffmpeg"));
    let ffmpeg_tool = FfmpegTool::from_path(
        system_path
            .find_binary("ffmpeg")
            .expect("ffmpeg not found in PATH")
            .to_path_buf(),
        system_path
            .find_binary("ffprobe")
            .expect("ffprobe not found in PATH")
            .to_path_buf(),
    );
    let resources = generate_sample_resources(line_mapping, &ffmpeg_tool).await?;
    let aud_file_task = {
        let resources = resources.clone();
        let output_dir = output_dir.to_path_buf();
        try_spawn_blocking(move || {
            let resource_aud_file = std::fs::File::create(output_dir.join("resource.aud"))?;
            let mut reader = resources.audio_volume().open_reader(..)?;
            std::io::copy(
                &mut reader,
                &mut std::io::BufWriter::new(&resource_aud_file),
            )?;
            Ok::<_, anyhow::Error>(())
        })
        .boxed()
    };
    let resource_tasks = futures::stream::iter(resources.map_resources().iter().cloned()).map({
        let output_dir = output_dir.to_path_buf();
        move |res| {
            try_spawn_blocking({
                let output_dir = output_dir.clone();
                move || {
                    let file = PathBuf::from(format!(
                        "{}.{}",
                        res.id().resource_num(),
                        res.id().type_id().to_file_ext()
                    ));
                    let open_file = std::fs::File::create(output_dir.join(&file))?;
                    res.write_patch(open_file)?;
                    Ok::<_, anyhow::Error>(())
                }
            })
            .boxed()
        }
    });

    resource_tasks
        .chain(futures::stream::iter(std::iter::once(aud_file_task)))
        .buffer_unordered(10)
        .try_collect::<()>()
        .await?;

    Ok(())
}

fn map_from_legacy_dir(base_dir: &Path) -> anyhow::Result<BTreeMap<LineId, AudioClip>> {
    legacy_load_dir(base_dir)
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

pub(crate) async fn compile_audio(
    scan_type: ScanType,
    sample_dir: &Path,
    output_dir: &Path,
) -> anyhow::Result<()> {
    let sample_set = match scan_type {
        ScanType::Legacy => map_from_legacy_dir(sample_dir)?,
        ScanType::Scannable => map_from_sample_scan(sample_dir)?,
    };

    compile_audio_base(&sample_set, output_dir).await?;
    Ok(())
}
