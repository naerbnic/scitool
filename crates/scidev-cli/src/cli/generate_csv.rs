use clap::Parser;
use std::path::PathBuf;

use crate::cmds::generate_csv::generate_csv;

#[derive(Parser)]
pub(super) struct GenerateCsv {
    book_path: PathBuf,

    /// Base URL for the game script page.
    #[clap(long, default_value = "https://sq5-fan-dub.github.io/script")]
    base_url: String,
}

impl GenerateCsv {
    pub(super) fn run(&self) -> anyhow::Result<()> {
        let csv = generate_csv(&self.book_path, &self.base_url)?;
        println!("{csv}");
        Ok(())
    }
}
