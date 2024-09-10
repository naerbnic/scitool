use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use crate::output::msg as msg_out;
use crate::res::msg as msg_res;
use crate::res::msg::MessageRecord;
use crate::res::{file::open_game_resources, msg::parse_message_resource, ResourceType};
use clap::{Parser, Subcommand};

// My current theory is that messages are separatable into a few categories:

struct ErrorContext {
    contexts: Vec<String>,
    errors: Vec<String>,
}

impl ErrorContext {
    fn new() -> Self {
        Self {
            contexts: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn with_context<R>(
        &mut self,
        context: impl Into<String>,
        body: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.contexts.push(context.into());
        let result = body(self);
        self.contexts.pop();
        result
    }

    fn add_error(&mut self, error: impl AsRef<str>) {
        let mut result = String::new();
        for context in &self.contexts {
            result.push_str(context);
        }
        result.push_str(error.as_ref());
        self.errors.push(result);
    }

    fn errors(&self) -> &[String] {
        &self.errors
    }
}

struct Line {
    #[expect(dead_code)]
    talker: u8,
    #[expect(dead_code)]
    text: String,
}

impl Line {
    fn from_message(message: &MessageRecord) -> Self {
        Self {
            talker: message.talker(),
            text: message.text().to_string(),
        }
    }
}

struct Conversation {
    lines: BTreeMap<u8, Line>,
}

impl Conversation {
    fn new() -> Self {
        Self {
            lines: BTreeMap::new(),
        }
    }

    fn insert_line(&mut self, id: &msg_res::MessageId, message: &MessageRecord) {
        self.lines
            .insert(id.sequence(), Line::from_message(message));
    }

    fn validate(&self, context: &mut ErrorContext) {
        // All conversations have lines from 1 to N.
        let expected_sequences = (1..=self.lines.len() as u8).collect::<Vec<_>>();
        let mut actual_sequences = self.lines.keys().cloned().collect::<Vec<_>>();
        actual_sequences.sort();
        if expected_sequences != actual_sequences {
            context.add_error(format!(
                "Expected sequences {:?}, got {:?}",
                expected_sequences, actual_sequences
            ));
        }
    }
}

struct ConversationSet {
    condition_conversations: BTreeMap<u8, Conversation>,
}

impl ConversationSet {
    fn new() -> Self {
        Self {
            condition_conversations: BTreeMap::new(),
        }
    }

    fn insert_message(&mut self, id: &msg_res::MessageId, message: &MessageRecord) {
        self.condition_conversations
            .entry(id.condition())
            .or_insert_with(Conversation::new)
            .insert_line(id, message);
    }

    fn validate(&self, context: &mut ErrorContext) {
        for (condition, conversation) in &self.condition_conversations {
            context.with_context(format!("cond: {} ", condition), |context| {
                conversation.validate(context)
            });
        }
    }

    fn conditions(&self) -> impl Iterator<Item = u8> + '_ {
        self.condition_conversations.keys().cloned()
    }
}

struct RoomNoun {
    verb_conversations: BTreeMap<u8, ConversationSet>,
}

impl RoomNoun {
    fn new() -> Self {
        Self {
            verb_conversations: BTreeMap::new(),
        }
    }

    fn insert_message(&mut self, id: &msg_res::MessageId, message: &MessageRecord) {
        self.verb_conversations
            .entry(id.verb())
            .or_insert_with(ConversationSet::new)
            .insert_message(id, message);
    }

    fn validate(&self, context: &mut ErrorContext) {
        for (verb, conversation_set) in &self.verb_conversations {
            context.with_context(format!("verb: {} ", verb), |context| {
                conversation_set.validate(context)
            });
        }
    }

    fn conditions(&self) -> impl Iterator<Item = u8> + '_ {
        self.verb_conversations
            .values()
            .flat_map(ConversationSet::conditions)
    }
}

struct MessageRoom {
    nouns: BTreeMap<u8, RoomNoun>,
}

impl MessageRoom {
    fn new() -> Self {
        Self {
            nouns: BTreeMap::new(),
        }
    }

    fn insert_message(&mut self, id: &msg_res::MessageId, message: &MessageRecord) {
        self.nouns
            .entry(id.noun())
            .or_insert_with(RoomNoun::new)
            .insert_message(id, message);
    }

    fn validate(&self, context: &mut ErrorContext) {
        // Scenes and nouns should be disjoint
        for (noun, room_noun) in &self.nouns {
            context.with_context(format!("noun: {} ", noun), |context| {
                room_noun.validate(context)
            });
        }
    }

    fn conditions(&self) -> BTreeSet<u8> {
        self.nouns.values().flat_map(RoomNoun::conditions).collect()
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
            let config: msg_out::ScriptConfig =
                serde_yml::from_reader(std::fs::File::open(config_path)?)?;
            eprintln!("Loaded config from {:?}: {:?}", config_path, config);
        }
        let resource_set = open_game_resources(&self.root_dir)?;

        // Extra testing for building a conversation.

        for (id, res) in resource_set.resources_of_type(ResourceType::Message) {
            let msg_resources = parse_message_resource(res.open()?)?;
            for (msg_id, record) in msg_resources.messages() {
                if let Some(room) = self.room {
                    if id.resource_num != room {
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
struct CheckMessages {
    #[clap(index = 1)]
    root_dir: PathBuf,
    #[clap(long = "config")]
    config_path: Option<PathBuf>,
}

impl CheckMessages {
    fn run(&self) -> anyhow::Result<()> {
        if let Some(config_path) = &self.config_path {
            let config: msg_out::ScriptConfig =
                serde_yml::from_reader(std::fs::File::open(config_path)?)?;
            eprintln!("Loaded config from {:?}: {:?}", config_path, config);
        }
        let resource_set = open_game_resources(&self.root_dir)?;

        // Extra testing for building a conversation.

        for (id, res) in resource_set.resources_of_type(ResourceType::Message) {
            let msg_resources = parse_message_resource(res.open()?)?;
            let mut msg_room = MessageRoom::new();
            for (msg_id, record) in msg_resources.messages() {
                msg_room.insert_message(msg_id, record);
            }
            let mut context = ErrorContext::new();
            msg_room.validate(&mut context);
            if context.errors().is_empty() {
                eprintln!("Room {}: OK", id.resource_num);
            } else {
                eprintln!("Room {}: {:#?}", id.resource_num, context.errors());
            }

            let conditions = msg_room.conditions();
            if conditions != (0..1).collect() {
                eprintln!("Room {}: Conditions: {:?}", id.resource_num, conditions);
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
