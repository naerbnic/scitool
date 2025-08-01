use std::path::PathBuf;

use clap::{Parser, Subcommand};

use scitool_cli::commands::generate::generate_csv;

#[derive(Parser)]
struct CommonArgs {
    /// Path to the book file.
    book_path: PathBuf,
}

#[derive(Parser)]
struct GenerateCsv {
    #[clap(flatten)]
    common: CommonArgs,

    /// Base URL for the game script page.
    #[clap(long, default_value = "https://sq5-fan-dub.github.io/script")]
    base_url: String,
}

impl GenerateCsv {
    fn run(&self) -> anyhow::Result<()> {
        let csv = generate_csv(&self.common.book_path, &self.base_url)?;
        println!("{csv}");
        Ok(())
    }
}

/// The specific generation command to execute.
#[derive(Subcommand)]
enum GenerateCommand {
    #[clap(about = "Generates a CSV representation of the game script.")]
    Csv(GenerateCsv),
}

/// Commands for generating different file formats from game data.
#[derive(Parser)]
pub(super) struct Generate {
    /// The specific generation command to execute.
    #[clap(subcommand)]
    msg_cmd: GenerateCommand,
}

impl Generate {
    pub(super) fn run(&self) -> anyhow::Result<()> {
        match &self.msg_cmd {
            GenerateCommand::Csv(cmd) => cmd.run(),
        }
    }
}
