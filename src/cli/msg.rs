use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::output::msg as msg_out;
use crate::res::{file::open_game_resources, msg::parse_message_resource, ResourceType};
use clap::{Parser, Subcommand};

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
        for (id, res) in resource_set.resources_of_type(ResourceType::Message) {
            let msg_resources = parse_message_resource(res.open()?)?;
            for (msg_id, record) in msg_resources.messages() {
                let message_id = {
                    msg_out::MessageId {
                        room: id.resource_num,
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
    #[clap(short = 't', long, required = false)]
    talker: Option<u8>,
}

impl PrintMessages {
    fn run(&self) -> anyhow::Result<()> {
        let resource_set = open_game_resources(&self.root_dir)?;
        for (id, res) in resource_set.resources_of_type(ResourceType::Message) {
            let msg_resources = parse_message_resource(res.open()?)?;
            for (msg_id, record) in msg_resources.messages() {
                if let Some(talker) = self.talker {
                    if record.talker() != talker {
                        continue;
                    }
                }
                println!(
                    "(room: {:?}, n: {:?}, v: {:?}, c: {:?}, s: {:?}, t: {:?}):",
                    id.resource_num,
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
struct PrintTalkers {
    #[clap(index = 1)]
    root_dir: PathBuf,
}

impl PrintTalkers {
    fn run(&self) -> anyhow::Result<()> {
        let resource_set = open_game_resources(&self.root_dir)?;
        let mut talkers = BTreeSet::new();
        for (_, res) in resource_set.resources_of_type(ResourceType::Message) {
            let msg_resources = parse_message_resource(res.open()?)?;
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
    PrintTalkers(PrintTalkers),
}

#[derive(Parser)]
pub struct Messages {
    #[clap(subcommand)]
    msg_cmd: MessageCommand,
}

impl Messages {
    pub fn run(&self) -> anyhow::Result<()> {
        match self.msg_cmd {
            MessageCommand::Export(ref cmd) => cmd.run()?,
            MessageCommand::Print(ref cmd) => cmd.run()?,
            MessageCommand::PrintTalkers(ref cmd) => cmd.run()?,
        }
        Ok(())
    }
}
