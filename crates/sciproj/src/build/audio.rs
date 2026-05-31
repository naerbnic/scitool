use std::{
    borrow::Cow,
    collections::BTreeMap,
    path::{Path, PathBuf},
    pin::Pin,
};

use crate::{
    book::Book,
    path::LookupPath,
    resources::{AudioClip, generate_sample_resources},
    tools::{espeak::EspeakTool, ffmpeg::FfmpegTool},
};
use futures_util::{StreamExt as _, TryStreamExt as _};
use indicatif::{MultiProgress, ProgressDrawTarget, ProgressFinish, ProgressStyle, TermLike};
use scidev::ids::LineId;

#[derive(Clone)]
pub struct ProgressFactory {
    multi_progress: MultiProgress,
    count_style: ProgressStyle,
    data_style: ProgressStyle,
    finish: ProgressFinish,
}

impl ProgressFactory {
    #[must_use]
    pub fn new<T>(term: T) -> Self
    where
        T: TermLike + 'static,
    {
        let draw_target = ProgressDrawTarget::term_like_with_hz(Box::new(term), 60);
        ProgressFactory {
            multi_progress: MultiProgress::with_draw_target(draw_target),
            count_style: ProgressStyle::with_template(
                "{prefix:>20!.bold}: {bar:40.cyan/blue} {human_pos:>9}/{human_len:9} {msg}",
            )
            .expect("is valid template"),
            data_style: ProgressStyle::with_template(
                "{prefix:>20!.bold}: {bar:40.cyan/blue} {decimal_bytes:>9}/{decimal_total_bytes:9} {msg}",
            )
            .expect("is valid template"),
            finish: ProgressFinish::AndClear,
        }
    }

    #[must_use]
    pub fn with_finish(mut self, finish: ProgressFinish) -> Self {
        self.finish = finish;
        self
    }

    #[must_use]
    pub fn make_count_bar(
        &self,
        len: u64,
        prefix: impl Into<Cow<'static, str>>,
    ) -> indicatif::ProgressBar {
        self.multi_progress.add(
            indicatif::ProgressBar::new(len)
                .with_prefix(prefix)
                .with_style(self.count_style.clone())
                .with_finish(self.finish.clone()),
        )
    }

    #[must_use]
    pub fn make_data_bar(
        &self,
        len: u64,
        prefix: impl Into<Cow<'static, str>>,
    ) -> indicatif::ProgressBar {
        self.multi_progress.add(
            indicatif::ProgressBar::new(len)
                .with_prefix(prefix)
                .with_style(self.data_style.clone())
                .with_finish(self.finish.clone()),
        )
    }
}

fn box_dyn_future<'a, F>(fut: F) -> Pin<Box<dyn Future<Output = F::Output> + 'a>>
where
    F: Future + 'a,
{
    Box::pin(fut)
}

pub async fn compile_audio_base(
    progress: ProgressFactory,
    book: &Book,
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
    let synth_tool = system_path
        .find_binary("espeak")
        .map(Path::to_path_buf)
        .map(EspeakTool::from_path);

    let resources = generate_sample_resources(
        progress.clone(),
        book,
        line_mapping,
        &ffmpeg_tool,
        synth_tool.as_ref(),
    )
    .await?;
    let aud_file_task = {
        let resources = resources.clone();
        let progress =
            progress.make_data_bar(resources.audio_volume().len(), "Writing audio volume");
        box_dyn_future(async move {
            tokio::io::copy(
                &mut progress.wrap_async_read(Box::into_pin(
                    resources.audio_volume().open_async_reader(..).await?,
                )),
                &mut tokio::fs::File::create(output_dir.join("resource.aud")).await?,
            )
            .await?;
            Ok(())
        })
    };
    let resource_tasks =
        futures_util::stream::iter(resources.map_resources().iter().cloned()).map({
            let output_dir = output_dir.to_path_buf();
            let progress = progress.make_count_bar(
                resources.map_resources().len().try_into().unwrap(),
                "Writing resources",
            );
            move |res| {
                let output_dir = output_dir.clone();
                let progress = progress.clone();
                box_dyn_future(async move {
                    let file = PathBuf::from(format!(
                        "{}.{}",
                        res.id().resource_num(),
                        res.id().type_id().to_file_ext()
                    ));
                    let open_file = tokio::fs::File::create(output_dir.join(&file)).await?;
                    res.write_patch_async(open_file).await?;
                    progress.inc(1);
                    progress.set_message(format!("Wrote resource {}", file.display()));
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
