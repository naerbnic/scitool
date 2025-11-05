use std::path::PathBuf;

use clap::Parser;
use clap::Subcommand;
use sciproj::book::{Book, Line, builder::BookBuilder, config::BookConfig, file_format};

use std::path::Path;

use scidev::resources::{ResourceSet, ResourceType, types::msg::parse_message_resource};

fn export_book(
    config_path: &Path,
    game_path: &Path,
    output: impl std::io::Write,
) -> anyhow::Result<()> {
    let config = if config_path.exists() {
        let config: BookConfig = serde_norway::from_reader(std::fs::File::open(config_path)?)?;
        config
    } else {
        BookConfig::default()
    };
    let resource_set = ResourceSet::from_root_dir(game_path)?;
    let mut builder = BookBuilder::new(config)?;

    // Extra testing for building a conversation.

    for res in resource_set.resources_of_type(ResourceType::Message) {
        let msg_resources = parse_message_resource(&res.data().open_mem(..)?)?;
        for (msg_id, record) in msg_resources.messages() {
            builder.add_message(res.id().resource_num(), msg_id, record)?;
        }
    }
    let book = builder.build()?;

    file_format::serialize_book(&book, &mut serde_json::Serializer::new(output))?;

    Ok(())
}

fn validate_book(book_path: &Path) -> anyhow::Result<()> {
    let book = file_format::deserialize_book(&mut serde_json::Deserializer::from_reader(
        std::fs::File::open(book_path)?,
    ))?;

    eprintln!(
        "Book loaded successfully with {} entries.",
        book.lines().count()
    );

    Ok(())
}

fn export_schema(pretty: bool) {
    let json_schema = file_format::json_schema(pretty);
    println!("{json_schema}");
}

struct LineFilter {
    /// Filter by talker ID.
    talker: Option<u8>,
    /// Filter by room ID.
    room: Option<u16>,
    /// Filter by verb ID.
    verb: Option<u8>,
    /// Filter by noun ID.
    noun: Option<u8>,
    /// Filter by condition ID.
    condition: Option<u8>,
    /// Filter by sequence ID.
    sequence: Option<u8>,
}

impl LineFilter {
    /// Create a new filter with all fields set to `None`.
    #[must_use]
    fn new(
        talker: Option<u8>,
        room: Option<u16>,
        verb: Option<u8>,
        noun: Option<u8>,
        condition: Option<u8>,
        sequence: Option<u8>,
    ) -> Self {
        Self {
            talker,
            room,
            verb,
            noun,
            condition,
            sequence,
        }
    }
}

fn for_each_line(
    book_path: &Path,
    filter: &LineFilter,
    mut body: impl for<'a> FnMut(&'a Line<'a>) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let book: Book = file_format::deserialize_book(&mut serde_json::Deserializer::from_reader(
        std::fs::File::open(book_path)?,
    ))?;
    for line in book.lines() {
        if let Some(room) = filter.room
            && line.id().room_num() != room
        {
            continue;
        }
        if let Some(talker) = filter.talker
            && line.talker_num() != talker
        {
            continue;
        }
        if let Some(verb) = filter.verb
            && line.id().verb_num() != verb
        {
            continue;
        }
        if let Some(noun) = filter.noun
            && line.id().noun_num() != noun
        {
            continue;
        }
        if let Some(condition) = filter.condition
            && line.id().condition_num() != condition
        {
            continue;
        }
        if let Some(sequence) = filter.sequence
            && line.id().sequence_num() != sequence
        {
            continue;
        }

        body(&line)?;
    }
    Ok(())
}

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
