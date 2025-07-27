use itertools::Itertools;
use std::{io::Write, path::PathBuf};

use clap::{Parser, Subcommand};
use scidev_resources::{ResourceType, file::open_game_resources, types::msg::parse_message_resource};
use scidev_book::{self as book, Book, builder::BookBuilder, config::BookConfig};

use crate::generate::{
    doc::{Document, DocumentBuilder, SectionBuilder},
    html::generate_html,
    json::GameScript,
    text::{RichText, TextStyle, make_conversation_title, make_noun_title, make_room_title},
};

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

fn generate_conversation(mut section: SectionBuilder, conversation: &book::Conversation) {
    section.set_id(conversation.id().to_string());
    let mut content = section.add_content();
    let mut dialogue = content.add_dialogue();
    for line in conversation.lines() {
        dialogue.add_line(
            line.role().short_name(),
            RichText::from_msg_text(line.text()),
            line.id().to_string(),
        );
    }
}

fn generate_document(book: &Book) -> Document {
    let mut doc = DocumentBuilder::new(format!("{} Script", book.project_name()));
    for room in book.rooms() {
        let mut room_title = RichText::builder();
        room_title.add_rich_text(&make_room_title(&room)).add_text(
            &format!(" (Room #{:?})", room.id().room_num()),
            &TextStyle::of_italic(),
        );
        let mut room_section = doc.add_chapter(room_title.build());
        room_section.set_id(room.id().to_string());
        let mut room_section = room_section.into_section_builder();

        for noun in room.nouns() {
            let num_conversations = noun.conversations().count();
            if num_conversations == 0 {
                continue;
            }

            let mut noun_section = room_section.add_subsection(make_noun_title(&noun));

            noun_section.set_id(noun.id().to_string());

            match noun.conversations().exactly_one() {
                Ok(conversation) => {
                    if let Some(verb) = conversation.verb() {
                        noun_section
                            .add_content()
                            .add_paragraph(format!("On {}", verb.name()));
                    }
                    generate_conversation(noun_section, &conversation);
                }
                Err(full_iter) => {
                    let mut noun_section_builder = noun_section.into_section_builder();

                    for conversation in full_iter {
                        let title = make_conversation_title(&conversation);
                        let conv_section = noun_section_builder.add_subsection(title);
                        generate_conversation(conv_section, &conversation);
                    }
                }
            }
        }
    }
    doc.build()
}

/// Generates a master HTML script document from the game book.
#[derive(Parser)]
struct GenerateMaster {
    /// Path to the game's root directory.
    #[clap(flatten)]
    ctxt: CommonArgs,
    /// Path to write the HTML output file.
    #[clap(short, long)]
    output: PathBuf,
}

impl GenerateMaster {
    fn run(&self) -> anyhow::Result<()> {
        let book = load_book(&self.ctxt)?;
        let doc = generate_document(&book);
        let html = generate_html(&doc);
        std::fs::write(&self.output, html)?;
        Ok(())
    }
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
    #[clap(about = "Generates a master HTML script document from the game book.")]
    Master(GenerateMaster),
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
            GenerateCommand::Master(cmd) => cmd.run(),
            GenerateCommand::Json(cmd) => cmd.run(),
            GenerateCommand::JsonSchema(cmd) => cmd.run(),
        }
    }
}
