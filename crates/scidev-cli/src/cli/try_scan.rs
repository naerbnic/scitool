use std::path::PathBuf;

use clap::Parser;

use crate::cmds::try_scan::try_scan;

#[derive(Parser)]
pub(crate) struct TryScan {
    #[clap(short = 's')]
    scan_dir: PathBuf,
}

impl TryScan {
    pub(crate) fn run(&self) -> anyhow::Result<()> {
        try_scan(&self.scan_dir)
    }
}
