use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    pin::Pin,
};

use crate::{
    path::LookupPath,
    resources::{AudioClip, generate_sample_resources},
    tools::ffmpeg::FfmpegTool,
};
use futures_util::{StreamExt as _, TryStreamExt as _};
use scidev::ids::LineId;

fn box_dyn_future<'a, F>(fut: F) -> Pin<Box<dyn Future<Output = F::Output> + 'a>>
where
    F: Future + 'a,
{
    Box::pin(fut)
}

pub async fn compile_audio_base(
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
    let resource_tasks =
        futures_util::stream::iter(resources.map_resources().iter().cloned()).map({
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
        .chain(futures_util::stream::iter(std::iter::once(aud_file_task)))
        .buffer_unordered(10)
        .try_collect::<()>()
        .await?;

    Ok(())
}
