use std::path::PathBuf;

use clap::Parser;
use clap::Subcommand;

use scitool_cli::commands::book::{export_book, export_schema, validate_book};

#[derive(Parser)]
pub(crate) struct BookCommand {
    #[clap(subcommand)]
    book_cmd: SubCommand,
}
impl BookCommand {
    pub(crate) fn run(&self) -> Result<(), anyhow::Error> {
        match &self.book_cmd {
            SubCommand::Export(export) => export.run()?,
            SubCommand::Validate(validate) => validate.run()?,
            SubCommand::Schema(schema) => schema.run(),
        }
        Ok(())
    }
}

#[derive(Subcommand)]
enum SubCommand {
    Export(ExportCommand),
    Validate(ValidateCommand),
    Schema(SchemaCommand),
}

#[derive(Parser)]
pub(crate) struct ExportCommand {
    config: PathBuf,
    game: PathBuf,
    /// Path to the output file. If not specified, writes to stdout.
    #[clap(long, short)]
    output: Option<PathBuf>,
}

impl ExportCommand {
    pub(crate) fn run(&self) -> anyhow::Result<()> {
        let output: Box<dyn std::io::Write> = if let Some(path) = &self.output {
            Box::new(std::fs::File::create_new(path)?)
        } else {
            Box::new(std::io::stdout().lock())
        };
        export_book(&self.config, &self.game, output)?;
        Ok(())
    }
}

#[derive(Parser)]
pub(crate) struct ValidateCommand {
    book: PathBuf,
}

impl ValidateCommand {
    pub(crate) fn run(&self) -> anyhow::Result<()> {
        validate_book(&self.book)?;
        Ok(())
    }
}

#[derive(Parser)]
struct SchemaCommand {
    #[clap(short, long, default_value = "false")]
    pretty: bool,
}

impl SchemaCommand {
    pub(crate) fn run(&self) {
        export_schema(self.pretty);
    }
}
