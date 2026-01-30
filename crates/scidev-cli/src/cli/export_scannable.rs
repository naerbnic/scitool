use std::path::PathBuf;

use clap::Parser;

use crate::cmds::export_scannable::export_scannable;

#[derive(Parser)]
pub(crate) struct ExportScannable {
    #[clap(short = 's')]
    sample_dir: PathBuf,

    #[clap(short = 'o', long)]
    output: PathBuf,
}

impl ExportScannable {
    pub(crate) async fn run(&self) -> anyhow::Result<()> {
        export_scannable(&self.sample_dir, &self.output).await
    }
}
