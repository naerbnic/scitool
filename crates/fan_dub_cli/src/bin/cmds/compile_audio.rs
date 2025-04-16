use std::path::PathBuf;

use clap::Parser;
use futures::{FutureExt, TryStreamExt, stream::FuturesUnordered};
use scitool_fan_dub_cli::{path::LookupPath, tools::ffmpeg::FfmpegTool};

async fn execute_all<F>(futures: impl IntoIterator<Item = F>) -> anyhow::Result<()>
where
    F: futures::Future<Output = anyhow::Result<()>> + Unpin,
{
    let mut fut_unordered = FuturesUnordered::from_iter(futures);
    while let Some(()) = fut_unordered.try_next().await? {
        // Do nothing
    }
    Ok(())
}

#[derive(Parser)]
pub struct CompileAudio {
    #[clap(short = 's')]
    sample_dir: PathBuf,

    #[clap(short = 'o', long)]
    output: PathBuf,
}

impl CompileAudio {
    pub async fn run(&self) -> anyhow::Result<()> {
        let system_path = LookupPath::from_env();
        eprintln!("System PATH: {:?}", system_path.find_binary("ffmpeg"));
        let ffmpeg_tool = FfmpegTool::from_path(
            system_path
                .find_binary("ffmpeg")
                .expect("ffmpeg not found in PATH")
                .to_path_buf(),
        );
        let sample_dir =
            scitool_fan_dub_cli::resources::SampleDir::load_dir(&self.sample_dir).await?;
        let resources = sample_dir.to_audio_resources(&ffmpeg_tool, 4).await?;

        let output_dir = &self.output;

        futures::try_join!(
            async {
                let resource_aud_file =
                    smol::fs::File::create(output_dir.join("resource.aud")).await?;
                resources
                    .audio_volume()
                    .write_to_async(resource_aud_file)
                    .await?;
                Ok::<_, anyhow::Error>(())
            },
            execute_all(resources.map_resources().iter().map(|res| {
                async move {
                    let file = PathBuf::from(format!(
                        "{}.{}",
                        res.id().resource_num(),
                        res.id().type_id().to_file_ext()
                    ));
                    let open_file = smol::fs::File::create(output_dir.join(&file)).await?;
                    res.write_patch(open_file).await?;
                    Ok::<_, anyhow::Error>(())
                }
                .boxed()
            }))
        )?;
        Ok(())
    }
}
