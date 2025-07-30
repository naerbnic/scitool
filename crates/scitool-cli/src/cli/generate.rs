use std::{io::Write, path::PathBuf};

use clap::{Parser, Subcommand};
use scidev_book::{Book, builder::BookBuilder, config::BookConfig};
use scidev_resources::{
    ResourceType, file::open_game_resources, types::msg::parse_message_resource,
};

use crate::generate::json::GameScript;

#[derive(Parser)]
struct CommonArgs {
    /// Path to the game's root directory.
    root_dir: PathBuf,
    /// Path to the book configuration YAML file.
    config_path: PathBuf,
}

fn load_book(args: &CommonArgs) -> anyhow::Result<Book> {
    let config = if args.config_path.exists() {
        let config: BookConfig = serde_yml::from_reader(std::fs::File::open(&args.config_path)?)?;
        config
    } else {
        BookConfig::default()
    };
    let resource_set = open_game_resources(&args.root_dir)?;
    let mut builder = BookBuilder::new(config)?;

    // Extra testing for building a conversation.

    for res in resource_set.resources_of_type(ResourceType::Message) {
        let msg_resources = parse_message_resource(&res.load_data()?)?;
        for (msg_id, record) in msg_resources.messages() {
            builder.add_message(res.id().resource_num(), msg_id, record)?;
        }
    }
    Ok(builder.build()?)
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
        }
    }
}
