use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
pub struct TryScan {
    #[clap(short = 's')]
    scan_dir: PathBuf,
}

impl TryScan {
    pub async fn run(&self) -> anyhow::Result<()> {
        let scan = scitool_fan_dub_cli::file::AudioSampleScan::read_from_dir(&self.scan_dir)?;

        anyhow::ensure!(
            !scan.has_duplicates(),
            "Duplicate files found in scan directory",
        );

        eprintln!("Scan directory: {:?}", scan.base_path());
        for (line_id, sample) in scan.get_valid_entries() {
            eprintln!("{}: {:?}", line_id, sample);
        }
        Ok(())
    }
}
