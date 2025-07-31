use std::{io::Write, path::PathBuf};

use clap::{Parser, Subcommand};
use scidev_book::Book;

use crate::generate::{csv::generate_csv, json::GameScript};

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

/// Generates a JSON representation of the game script.
#[derive(Parser)]
struct GenerateJson {
    /// Path to the game's root directory.
    #[clap(flatten)]
    ctxt: CommonArgs,
    /// Path to write the JSON output file.
    #[clap(short, long)]
    output: PathBuf,
}

impl GenerateJson {
    fn run(&self) -> anyhow::Result<()> {
        let book = load_book(&self.ctxt)?;
        let script = GameScript::from_book(&book);
        let json = serde_json::to_string(&script)?;
        std::fs::write(&self.output, json)?;
        Ok(())
    }
}

/// Generates the JSON schema for the game script structure and prints it to stdout.
#[derive(Parser)]
struct GenerateJsonSchema;

impl GenerateJsonSchema {
    #[expect(
        clippy::unused_self,
        reason = "Future-proofing for potential arguments"
    )]
    fn run(&self) -> anyhow::Result<()> {
        let schema = GameScript::json_schema()?;
        std::io::stdout().write_all(schema.as_bytes())?;
        Ok(())
    }
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
    #[clap(about = "Generates a JSON representation of the game script.")]
    Json(GenerateJson),
    #[clap(
        name = "json-schema",
        about = "Generates the JSON schema for the game script structure and prints it to stdout."
    )]
    JsonSchema(GenerateJsonSchema),
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
            GenerateCommand::Json(cmd) => cmd.run(),
            GenerateCommand::JsonSchema(cmd) => cmd.run(),
            GenerateCommand::Csv(cmd) => cmd.run(),
        }
    }
}
