use itertools::Itertools;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use sci_resources::{ResourceType, file::open_game_resources, types::msg::parse_message_resource};

use crate::{
    book::{
        Book, Control, FontControl, MessageSegment, MessageText, builder::BookBuilder,
        config::BookConfig,
    },
    generate::{
        doc::{
            Document, DocumentBuilder, SectionBuilder,
            text::{RichText, TextStyle},
        },
        html::generate_html,
    },
};

#[derive(Parser)]
struct CommonArgs {
    root_dir: PathBuf,
    config_path: PathBuf,
}

fn convert_message_text_to_rich_text(text: &MessageText) -> RichText {
    let mut builder = RichText::builder();
    let mut curr_style = TextStyle::default();
    for segment in text.segments() {
        match segment {
            MessageSegment::Text(text) => {
                builder.add_text(text, &curr_style);
            }
            MessageSegment::Control(ctrl) => match ctrl {
                Control::Font(font_ctrl) => match font_ctrl {
                    FontControl::Default => curr_style = TextStyle::default(),
                    FontControl::Italics => {
                        // Italics
                        curr_style = TextStyle::default();
                        curr_style.set_italic(true);
                    }
                    // Bold Controls
                    FontControl::SuperLarge | FontControl::Title | FontControl::BoldLike => {
                        // Super Large Font
                        curr_style = TextStyle::default();
                        curr_style.set_bold(true);
                    }
                    // Ignored
                    FontControl::Lowercase | FontControl::Unknown => {}
                },
                Control::Color(_) => {
                    // We ignore color control sequences for now.
                }
            },
        }
    }
    builder.build()
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
            convert_message_text_to_rich_text(line.text()),
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
        // build the room title, including the room number (for easy reference).
        let mut room_title_builder = RichText::builder();
        room_title_builder.add_plain_text(room.name()).add_text(
            format!(" (Room #{:?})", room.id().room_num()),
            TextStyle::default().set_italic(true),
        );
        let mut room_section = doc.add_chapter(room_title_builder.build());
        room_section.set_id(room_id_to_id_string(room.id()));
        let mut room_section = room_section.into_section_builder();

        for noun in room.nouns() {
            let num_conversations = noun.conversations().count();
            if num_conversations == 0 {
                continue;
            }
            let mut noun_desc = noun
                .desc()
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| format!("Noun #{:?}", noun.id().noun_num()));

            if noun.is_cutscene() {
                noun_desc.push_str(" (Cutscene)");
            }

            let mut noun_section = room_section.add_subsection(noun_desc);

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
                        let title =
                            match (conversation.verb(), conversation.condition()) {
                                (Some(verb), Some(cond)) => format!(
                                    "On {} ({})",
                                    verb.name(),
                                    cond.desc().map(ToString::to_string).unwrap_or_else(
                                        || format!("Condition #{:?}", cond.id().condition_num())
                                    )
                                ),
                                (Some(verb), None) => format!("On {}", verb.name()),
                                (None, Some(cond)) => format!(
                                    "When {}",
                                    cond.desc().map(ToString::to_string).unwrap_or_else(
                                        || format!("Condition #{:?}", cond.id().condition_num())
                                    )
                                ),
                                (None, None) => "On Any".to_string(),
                            };
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

#[derive(Subcommand)]
enum GenerateCommand {
    Master(GenerateMaster),
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
        }
    }
}
