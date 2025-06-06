use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
pub struct ExportScannable {
    #[clap(short = 's')]
    sample_dir: PathBuf,

    #[clap(short = 'o', long)]
    output: PathBuf,
}

impl ExportScannable {
    pub async fn run(&self) -> anyhow::Result<()> {
        let sample_dir =
            scitool_fan_dub_cli::resources::SampleDir::load_dir(&self.sample_dir).await?;
        sample_dir.save_to_scannable_dir(&self.output).await?;
        Ok(())
    }
}
