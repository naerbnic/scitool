use std::path::PathBuf;

use clap::Parser;
use rayon::iter::{IntoParallelRefIterator as _, ParallelIterator as _};
use scidub_cli::{
    file::AudioSampleScan, path::LookupPath, resources::SampleDir, tools::ffmpeg::FfmpegTool,
};

#[derive(clap::ValueEnum, Copy, Clone, Debug, Default)]
enum ScanType {
    #[clap(name = "legacy")]
    #[default]
    Legacy,
    #[clap(name = "scannable")]
    Scannable,
}

#[derive(Parser)]
pub(crate) struct CompileAudio {
    #[clap(
        short = 't',
        long,
        value_enum,
        required = false,
        default_value = "legacy"
    )]
    scan_type: ScanType,

    #[clap(short = 's')]
    sample_dir: PathBuf,

    #[clap(short = 'o', long)]
    output: PathBuf,
}

impl CompileAudio {
    pub(crate) fn run(&self) -> anyhow::Result<()> {
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
        let sample_dir = match self.scan_type {
            ScanType::Legacy => SampleDir::load_dir(&self.sample_dir)?,
            ScanType::Scannable => {
                let scan = AudioSampleScan::read_from_dir(&self.sample_dir)?;
                anyhow::ensure!(
                    !scan.has_duplicates(),
                    "Duplicate files found in scan directory",
                );
                SampleDir::from_sample_scan(&scan)?
            }
        };
        let resources = sample_dir.to_audio_resources(&ffmpeg_tool)?;

        let output_dir = &self.output;

        let aud_file_task = std::thread::spawn({
            let resources = resources.clone();
            let output_dir = output_dir.clone();
            move || {
                let resource_aud_file = std::fs::File::create(output_dir.join("resource.aud"))?;
                let mut reader = resources.audio_volume().open_reader(..)?;
                std::io::copy(
                    &mut reader,
                    &mut std::io::BufWriter::new(&resource_aud_file),
                )?;
                Ok::<_, anyhow::Error>(())
            }
        });

        resources
            .map_resources()
            .par_iter()
            .map(|res| {
                let output_dir = output_dir.clone();
                let file = PathBuf::from(format!(
                    "{}.{}",
                    res.id().resource_num(),
                    res.id().type_id().to_file_ext()
                ));
                let open_file = std::fs::File::create(output_dir.join(&file))?;
                res.write_patch(open_file)?;
                Ok::<_, anyhow::Error>(())
            })
            .collect::<anyhow::Result<()>>()?;

        aud_file_task.join().unwrap()?;

        Ok(())
    }
}
