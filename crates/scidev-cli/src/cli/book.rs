use std::path::PathBuf;

use clap::Parser;
use clap::Subcommand;

use crate::cmds::book::LineFilter;
use crate::cmds::book::export_book;
use crate::cmds::book::export_schema;
use crate::cmds::book::for_each_line;
use crate::cmds::book::validate_book;

#[derive(Parser)]
pub(super) struct BookCommand {
    #[clap(subcommand)]
    book_cmd: SubCommand,
}
impl BookCommand {
    pub(super) fn run(&self) -> Result<(), anyhow::Error> {
        match &self.book_cmd {
            SubCommand::Export(export) => export.run()?,
            SubCommand::Validate(validate) => validate.run()?,
            SubCommand::Schema(schema) => schema.run(),
            SubCommand::PrintMessages(print_messages) => print_messages.run()?,
        }
        Ok(())
    }
}

#[derive(Subcommand)]
enum SubCommand {
    /// Exports the book to a file or stdout.
    Export(ExportCommand),

    /// Validates the book file for correctness.
    Validate(ValidateCommand),

    /// Exports the schema for the book format.
    Schema(SchemaCommand),

    PrintMessages(PrintMessages),
}

#[derive(Parser)]
struct ExportCommand {
    /// Path to the configuration file.
    config: PathBuf,

    /// Path to the game's root directory.
    game: PathBuf,

    /// Path to the output file. If not specified, writes to stdout.
    #[clap(long, short)]
    output: Option<PathBuf>,
}

impl ExportCommand {
    fn run(&self) -> anyhow::Result<()> {
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
struct ValidateCommand {
    /// Path to the book file to validate.
    book: PathBuf,
}

impl ValidateCommand {
    fn run(&self) -> anyhow::Result<()> {
        validate_book(&self.book)?;
        Ok(())
    }
}

#[derive(Parser)]
struct SchemaCommand {
    /// If set, pretty-prints the schema output.
    #[clap(short, long, default_value = "false")]
    pretty: bool,
}

impl SchemaCommand {
    fn run(&self) {
        export_schema(self.pretty);
    }
}

/// Prints messages from the game, with optional filters.
#[derive(Parser)]
struct PrintMessages {
    /// Path to the book file.
    book_path: PathBuf,

    /// Filter by talker ID.
    #[clap(short = 't', long, required = false)]
    talker: Option<u8>,

    /// Filter by room ID.
    #[clap(short = 'r', long, required = false)]
    room: Option<u16>,

    /// Filter by verb ID.
    #[clap(short = 'v', long, required = false)]
    verb: Option<u8>,

    /// Filter by noun ID.
    #[clap(short = 'n', long, required = false)]
    noun: Option<u8>,

    /// Filter by condition ID.
    #[clap(short = 'c', long, required = false)]
    condition: Option<u8>,

    /// Filter by sequence ID.
    #[clap(short = 's', long, required = false)]
    sequence: Option<u8>,
}

impl PrintMessages {
    fn run(&self) -> anyhow::Result<()> {
        let filter = LineFilter::new(
            self.talker,
            self.room,
            self.verb,
            self.noun,
            self.condition,
            self.sequence,
        );
        for_each_line(&self.book_path, &filter, |line| {
            println!(
                "(room: {:?}, n: {:?}, v: {:?}, c: {:?}, s: {:?}, t: {:?}):",
                line.id().room_num(),
                line.id().noun_num(),
                line.id().verb_num(),
                line.id().condition_num(),
                line.id().sequence_num(),
                line.talker_num(),
            );
            let text = line.text().to_plain_text().replace("\r\n", "\n    ");
            println!("    {}", text.trim());
            Ok(())
        })?;
        Ok(())
    }
}
