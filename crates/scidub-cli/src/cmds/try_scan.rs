use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
pub(crate) struct TryScan {
    #[clap(short = 's')]
    scan_dir: PathBuf,
}

impl TryScan {
    pub(crate) fn run(&self) -> anyhow::Result<()> {
        let scan = scidub::file::AudioSampleScan::read_from_dir(&self.scan_dir)?;

        anyhow::ensure!(
            !scan.has_duplicates(),
            "Duplicate files found in scan directory",
        );

        eprintln!("Scan directory: {}", scan.base_path().display());
        for (line_id, sample) in scan.get_valid_entries() {
            eprintln!("{line_id}: {sample:?}");
        }
        Ok(())
    }
}
