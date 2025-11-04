use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
pub(crate) struct ExportScannable {
    #[clap(short = 's')]
    sample_dir: PathBuf,

    #[clap(short = 'o', long)]
    output: PathBuf,
}

impl ExportScannable {
    pub(crate) async fn run(&self) -> anyhow::Result<()> {
        let sample_dir = scidub::resources::SampleDir::load_dir(&self.sample_dir)?;
        sample_dir.save_to_scannable_dir(&self.output).await?;
        Ok(())
    }
}
