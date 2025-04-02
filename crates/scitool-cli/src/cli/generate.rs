use itertools::Itertools;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use sci_resources::{ResourceType, file::open_game_resources, types::msg::parse_message_resource};

use crate::{
    book::{Book, builder::BookBuilder, config::BookConfig},
    generate::{
        doc::{Document, DocumentBuilder, SectionBuilder},
        html::generate_html,
        json::GameScript,
        text::{RichText, make_conversation_title, make_noun_title, make_room_title},
    },
};

#[derive(Parser)]
struct CommonArgs {
    root_dir: PathBuf,
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
        let msg_resources = parse_message_resource(res.load_data()?)?;
        for (msg_id, record) in msg_resources.messages() {
            builder.add_message(res.id().resource_num(), msg_id, record)?;
        }
    }
    Ok(builder.build()?)
}

fn generate_conversation(mut section: SectionBuilder, conversation: &crate::book::Conversation) {
    section.set_id(conversation_id_to_id_string(conversation.id()));
    let mut content = section.add_content();
    let mut dialogue = content.add_dialogue();
    for line in conversation.lines() {
        dialogue.add_line(
            line.role().short_name(),
            RichText::from_msg_text(line.text()),
            line_id_to_id_string(line.id()),
        );
    }
}

fn room_id_to_id_string(room_id: crate::book::RoomId) -> String {
    format!("room-{}", room_id.room_num())
}

fn noun_id_to_id_string(noun_id: crate::book::NounId) -> String {
    format!("noun-{}-{}", noun_id.room_num(), noun_id.noun_num())
}

fn conversation_id_to_id_string(conversation_id: crate::book::ConversationId) -> String {
    format!(
        "conv-{}-{}-{}-{}",
        conversation_id.room_num(),
        conversation_id.noun_num(),
        conversation_id.verb_num(),
        conversation_id.condition_num(),
    )
}

fn line_id_to_id_string(line_id: crate::book::LineId) -> String {
    format!(
        "line-{}-{}-{}-{}-{}",
        line_id.room_num(),
        line_id.noun_num(),
        line_id.verb_num(),
        line_id.condition_num(),
        line_id.sequence_num(),
    )
}

fn generate_document(book: &Book) -> anyhow::Result<Document> {
    let mut doc = DocumentBuilder::new(format!("{} Script", book.project_name()));
    for room in book.rooms() {
        let mut room_section = doc.add_chapter(make_room_title(&room));
        room_section.set_id(room_id_to_id_string(room.id()));
        let mut room_section = room_section.into_section_builder();

        for noun in room.nouns() {
            let num_conversations = noun.conversations().count();
            if num_conversations == 0 {
                continue;
            }

            let mut noun_section = room_section.add_subsection(make_noun_title(&noun));

            noun_section.set_id(noun_id_to_id_string(noun.id()));

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
    Ok(doc.build())
}

#[derive(Parser)]
struct GenerateMaster {
    #[clap(flatten)]
    ctxt: CommonArgs,
    #[clap(short, long)]
    output: PathBuf,
}

impl GenerateMaster {
    fn run(&self) -> anyhow::Result<()> {
        let book = load_book(&self.ctxt)?;
        let doc = generate_document(&book)?;
        let html = generate_html(&doc)?;
        std::fs::write(&self.output, html)?;
        Ok(())
    }
}

#[derive(Parser)]
struct GenerateJson {
    #[clap(flatten)]
    ctxt: CommonArgs,
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

#[derive(Subcommand)]
enum GenerateCommand {
    Master(GenerateMaster),
    Json(GenerateJson),
}

#[derive(Parser)]
pub struct Generate {
    #[clap(subcommand)]
    msg_cmd: GenerateCommand,
}

impl Generate {
    pub fn run(&self) -> anyhow::Result<()> {
        match &self.msg_cmd {
            GenerateCommand::Master(cmd) => cmd.run(),
            GenerateCommand::Json(cmd) => cmd.run(),
        }
    }
}
