use std::collections::BTreeSet;
use std::path::PathBuf;

use scitool_book::{builder::BookBuilder, config::BookConfig};

use crate::output::msg as msg_out;
use clap::{Parser, Subcommand};
use sci_resources::{ResourceType, file::open_game_resources, types::msg::parse_message_resource};

// My current theory is that messages are separatable into a few categories:

#[derive(Parser)]
struct ExportMessages {
    #[clap(index = 1)]
    root_dir: PathBuf,
    #[clap(short = 'o', long)]
    output: PathBuf,
}

impl ExportMessages {
    fn run(&self) -> anyhow::Result<()> {
        let resource_set = open_game_resources(&self.root_dir)?;
        let mut messages = Vec::new();
        for res in resource_set.resources_of_type(ResourceType::Message) {
            let msg_resources = parse_message_resource(res.load_data()?)?;
            for (msg_id, record) in msg_resources.messages() {
                let message_id = {
                    msg_out::MessageId {
                        room: res.id().resource_num(),
                        noun: msg_id.noun(),
                        verb: msg_id.verb(),
                        condition: msg_id.condition(),
                        sequence: msg_id.sequence(),
                    }
                };
                let message = msg_out::Message {
                    id: message_id,
                    talker: record.talker(),
                    text: record.text().to_string(),
                };
                messages.push(message);
            }
        }

        eprintln!("Writing {:?} messages to {:?}", messages.len(), self.output);

        let msg_file = msg_out::MessageFile { messages };
        let writer = std::fs::File::create(&self.output)?;
        serde_json::to_writer_pretty(writer, &msg_file)?;
        Ok(())
    }
}

#[derive(Parser)]
struct PrintMessages {
    #[clap(index = 1)]
    root_dir: PathBuf,
    #[clap(long = "config")]
    config_path: Option<PathBuf>,
    #[clap(short = 't', long, required = false)]
    talker: Option<u8>,
    #[clap(short = 'r', long, required = false)]
    room: Option<u16>,
    #[clap(short = 'v', long, required = false)]
    verb: Option<u8>,
    #[clap(short = 'n', long, required = false)]
    noun: Option<u8>,
    #[clap(short = 'c', long, required = false)]
    condition: Option<u8>,
    #[clap(short = 's', long, required = false)]
    sequence: Option<u8>,
}

impl PrintMessages {
    fn run(&self) -> anyhow::Result<()> {
        if let Some(config_path) = &self.config_path {
            let config: BookConfig = serde_yml::from_reader(std::fs::File::open(config_path)?)?;
            eprintln!("Loaded config from {:?}: {:?}", config_path, config);
        }
        let resource_set = open_game_resources(&self.root_dir)?;

        // Extra testing for building a conversation.

        for res in resource_set.resources_of_type(ResourceType::Message) {
            let msg_resources = parse_message_resource(res.load_data()?)?;
            for (msg_id, record) in msg_resources.messages() {
                if let Some(room) = self.room {
                    if res.id().resource_num() != room {
                        continue;
                    }
                }
                if let Some(talker) = self.talker {
                    if record.talker() != talker {
                        continue;
                    }
                }
                if let Some(verb) = self.verb {
                    if msg_id.verb() != verb {
                        continue;
                    }
                }
                if let Some(noun) = self.noun {
                    if msg_id.noun() != noun {
                        continue;
                    }
                }
                if let Some(condition) = self.condition {
                    if msg_id.condition() != condition {
                        continue;
                    }
                }
                if let Some(sequence) = self.sequence {
                    if msg_id.sequence() != sequence {
                        continue;
                    }
                }
                println!(
                    "(room: {:?}, n: {:?}, v: {:?}, c: {:?}, s: {:?}, t: {:?}):",
                    res.id().resource_num(),
                    msg_id.noun(),
                    msg_id.verb(),
                    msg_id.condition(),
                    msg_id.sequence(),
                    record.talker(),
                );
                let text = record.text().replace("\r\n", "\n    ");
                println!("    {}", text.trim());
            }
        }
        Ok(())
    }
}

#[derive(Parser)]
struct CheckMessages {
    #[clap(index = 1)]
    root_dir: PathBuf,
    #[clap(long = "config")]
    config_path: Option<PathBuf>,
}

impl CheckMessages {
    fn run(&self) -> anyhow::Result<()> {
        let config = if let Some(config_path) = &self.config_path {
            let config: BookConfig = serde_yml::from_reader(std::fs::File::open(config_path)?)?;
            eprintln!("Loaded config from {:?}", config_path);
            config
        } else {
            BookConfig::default()
        };
        let resource_set = open_game_resources(&self.root_dir)?;
        let mut builder = BookBuilder::new(config)?;

        // Extra testing for building a conversation.

        for res in resource_set.resources_of_type(ResourceType::Message) {
            let msg_resources = parse_message_resource(res.load_data()?)?;
            for (msg_id, record) in msg_resources.messages() {
                builder.add_message(res.id().resource_num(), msg_id, record)?;
            }
        }
        let book = builder.build()?;

        eprintln!("Num rooms: {}", book.rooms().count());
        eprintln!("Num nouns: {}", book.nouns().count());
        eprintln!("Num conversations: {}", book.conversations().count());
        eprintln!(
            "Num multi-line conversations: {}",
            book.conversations()
                .filter(|c| c.lines().count() > 1)
                .count()
        );
        eprintln!("Num lines: {}", book.lines().count());
        eprintln!(
            "Num empty lines: {}",
            book.lines().filter(|line| line.text().is_empty()).count()
        );

        for conversation in book.conversations() {
            if let Err(e) = conversation.validate_complete() {
                eprintln!("Conversation {:?}: {}", conversation.id(), e);
            }
        }

        for room in book.rooms() {
            eprintln!("Room {:?}:", room.name(),);
            eprintln!("  Num Conditions: {}", room.conditions().count());
        }
        Ok(())
    }
}

#[derive(Parser)]
struct PrintTalkers {
    #[clap(index = 1)]
    root_dir: PathBuf,
}

impl PrintTalkers {
    fn run(&self) -> anyhow::Result<()> {
        let resource_set = open_game_resources(&self.root_dir)?;
        let mut talkers = BTreeSet::new();
        for res in resource_set.resources_of_type(ResourceType::Message) {
            let msg_resources = parse_message_resource(res.load_data()?)?;
            for (_, record) in msg_resources.messages() {
                talkers.insert(record.talker());
            }
        }
        println!("Talkers:");
        println!(
            "  {}",
            talkers
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        );
        Ok(())
    }
}

#[derive(Subcommand)]
enum MessageCommand {
    Export(ExportMessages),
    Print(PrintMessages),
    Check(CheckMessages),
    PrintTalkers(PrintTalkers),
}

#[derive(Parser)]
pub struct Messages {
    #[clap(subcommand)]
    msg_cmd: MessageCommand,
}

impl Messages {
    pub fn run(&self) -> anyhow::Result<()> {
        match &self.msg_cmd {
            MessageCommand::Export(cmd) => cmd.run()?,
            MessageCommand::Print(cmd) => cmd.run()?,
            MessageCommand::Check(cmd) => cmd.run()?,
            MessageCommand::PrintTalkers(cmd) => cmd.run()?,
        }
        Ok(())
    }
}
