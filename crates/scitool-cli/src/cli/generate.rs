use std::path::PathBuf;

use clap::{Parser, Subcommand};
use scidev_book::Book;

use crate::generate::csv::generate_csv;

#[derive(Parser)]
struct CommonArgs {
    /// Path to the book file.
    book_path: PathBuf,
}

fn load_book(args: &CommonArgs) -> anyhow::Result<Book> {
    let book: Book = scidev_book::file_format::deserialize_book(
        &mut serde_json::Deserializer::from_reader(std::fs::File::open(&args.book_path)?),
    )?;
    Ok(book)
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
        let book = load_book(&self.common)?;
        let csv = generate_csv(&book, &self.base_url)?;
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
pub(crate) struct Generate {
    /// The specific generation command to execute.
    #[clap(subcommand)]
    msg_cmd: GenerateCommand,
}

impl Generate {
    pub(crate) fn run(&self) -> anyhow::Result<()> {
        match &self.msg_cmd {
            GenerateCommand::Csv(cmd) => cmd.run(),
        }
    }
}
