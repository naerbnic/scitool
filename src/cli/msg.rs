use std::path::PathBuf;

use crate::output::msg as msg_out;
use crate::res::{file::open_game_resources, msg::parse_message_resource, ResourceType};
use clap::{Parser, Subcommand};

#[derive(Parser)]
struct ReadMessages {
    #[clap(index = 1)]
    root_dir: PathBuf,
}

impl ReadMessages {
    fn run(&self) -> anyhow::Result<()> {
        let resource_set = open_game_resources(&self.root_dir)?;
        for (id, res) in resource_set.resources_of_type(ResourceType::Message) {
            eprintln!("Reading message {:?}", id);
            parse_message_resource(res.open()?)?;
        }
        Ok(())
    }
}

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

        let msg_file = msg_out::MessageFile { messages };
        let writer = std::fs::File::create(&self.output)?;
        serde_json::to_writer_pretty(writer, &msg_file)?;
        Ok(())
    }
}

#[derive(Subcommand)]
enum MessageCommand {
    Read(ReadMessages),
    Export(ExportMessages),
}

#[derive(Parser)]
pub struct Messages {
    #[clap(subcommand)]
    msg_cmd: MessageCommand,
}

impl Messages {
    pub fn run(&self) -> anyhow::Result<()> {
        match self.msg_cmd {
            MessageCommand::Read(ref cmd) => cmd.run()?,
            MessageCommand::Export(ref cmd) => cmd.run()?,
        }
        Ok(())
    }
}
