use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    pin::Pin,
};

use futures::prelude::*;
use scidev::ids::LineId;
use sciproj::{
    path::LookupPath,
    resources::{AudioClip, ScanType, generate_sample_resources, load_config_from_directory},
    tools::ffmpeg::FfmpegTool,
};

fn box_dyn_future<'a, F>(fut: F) -> Pin<Box<dyn Future<Output = F::Output> + 'a>>
where
    F: Future + 'a,
{
    Box::pin(fut)
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
    );
    let resources = generate_sample_resources(line_mapping, &ffmpeg_tool).await?;
    let aud_file_task = {
        let resources = resources.clone();
        box_dyn_future(async move {
            tokio::io::copy(
                &mut Box::into_pin(resources.audio_volume().open_async_reader(..).await?),
                &mut tokio::fs::File::create(output_dir.join("resource.aud")).await?,
            )
            .await?;
            Ok(())
        })
    };
    let resource_tasks = futures::stream::iter(resources.map_resources().iter().cloned()).map({
        let output_dir = output_dir.to_path_buf();
        move |res| {
            let output_dir = output_dir.clone();
            box_dyn_future(async move {
                let file = PathBuf::from(format!(
                    "{}.{}",
                    res.id().resource_num(),
                    res.id().type_id().to_file_ext()
                ));
                let open_file = tokio::fs::File::create(output_dir.join(&file)).await?;
                res.write_patch_async(open_file).await?;
                Ok::<_, anyhow::Error>(())
            })
        }
    });

    resource_tasks
        .chain(futures::stream::iter(std::iter::once(aud_file_task)))
        .buffer_unordered(10)
        .try_collect::<()>()
        .await?;

    Ok(())
}

pub(crate) async fn compile_audio(
    scan_type: ScanType,
    sample_dir: &Path,
    output_dir: &Path,
) -> anyhow::Result<()> {
    let sample_set = load_config_from_directory(scan_type, sample_dir)?;

    compile_audio_base(&sample_set, output_dir).await?;
    Ok(())
}
