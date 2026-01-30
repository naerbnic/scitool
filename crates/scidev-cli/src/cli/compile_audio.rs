use std::path::PathBuf;

use crate::cmds::compile_audio::{ScanType, compile_audio};
use clap::Parser;

#[derive(clap::ValueEnum, Copy, Clone, Debug, Default)]
enum ScanTypeFlag {
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
    scan_type: ScanTypeFlag,

    #[clap(short = 's')]
    sample_dir: PathBuf,

    #[clap(short = 'o', long)]
    output: PathBuf,
}

impl CompileAudio {
    pub(crate) fn run(&self) -> anyhow::Result<()> {
        compile_audio(
            match self.scan_type {
                ScanTypeFlag::Legacy => ScanType::Legacy,
                ScanTypeFlag::Scannable => ScanType::Scannable,
            },
            &self.sample_dir,
            &self.output,
        )
    }
}
