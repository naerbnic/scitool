use std::path::PathBuf;

use clap::Parser;
use clap::Subcommand;
use scidev_book::builder::BookBuilder;
use scidev_book::config::BookConfig;
use scidev_resources::ResourceType;
use scidev_resources::file::open_game_resources;
use scidev_resources::types::msg::parse_message_resource;

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
struct ExportCommand {
    config: PathBuf,
    game: PathBuf,
    /// Path to the output file. If not specified, writes to stdout.
    #[clap(long, short)]
    output: Option<PathBuf>,
}

impl ExportCommand {
    fn run(&self) -> anyhow::Result<()> {
        let config = if self.config.exists() {
            let config: BookConfig = serde_yml::from_reader(std::fs::File::open(&self.config)?)?;
            config
        } else {
            BookConfig::default()
        };
        let resource_set = open_game_resources(&self.game)?;
        let mut builder = BookBuilder::new(config)?;

        // Extra testing for building a conversation.

        for res in resource_set.resources_of_type(ResourceType::Message) {
            let msg_resources = parse_message_resource(&res.load_data()?)?;
            for (msg_id, record) in msg_resources.messages() {
                builder.add_message(res.id().resource_num(), msg_id, record)?;
            }
        }
        let book = builder.build()?;

        let output: Box<dyn std::io::Write> = if let Some(path) = &self.output {
            Box::new(std::fs::File::open(path)?)
        } else {
            Box::new(std::io::stdout().lock())
        };

        scidev_book::file_format::serialize_book(&book, &mut serde_json::Serializer::new(output))?;

        Ok(())
    }
}

#[derive(Parser)]
struct ValidateCommand {
    book: PathBuf,
}

impl ValidateCommand {
    fn run(&self) -> anyhow::Result<()> {
        let book = scidev_book::file_format::deserialize_book(
            &mut serde_json::Deserializer::from_reader(std::fs::File::open(&self.book)?),
        )?;

        eprintln!(
            "Book loaded successfully with {} entries.",
            book.lines().count()
        );

        Ok(())
    }
}

#[derive(Parser)]
struct SchemaCommand {
    #[clap(short, long, default_value = "false")]
    pretty: bool,
}

impl SchemaCommand {
    fn run(&self) {
        let json_schema = scidev_book::file_format::json_schema(self.pretty);
        println!("{json_schema}");
    }
}
