use std::collections::BTreeSet;
use std::path::PathBuf;

use scidev_book::Book;

use clap::{Parser, Subcommand};
use scidev_resources::{
    ResourceType, file::open_game_resources, types::msg::parse_message_resource,
};

/// Prints messages from the game, with optional filters.
#[derive(Parser)]
struct PrintMessages {
    /// Path to the book file.
    #[clap(index = 1)]
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
        let book: Book = scidev_book::file_format::deserialize_book(
            &mut serde_json::Deserializer::from_reader(std::fs::File::open(&self.book_path)?),
        )?;

        // Extra testing for building a conversation.

        for line in book.lines() {
            if let Some(room) = self.room {
                if line.id().room_num() != room {
                    continue;
                }
            }
            if let Some(talker) = self.talker {
                if line.talker_num() != talker {
                    continue;
                }
            }
            if let Some(verb) = self.verb {
                if line.id().verb_num() != verb {
                    continue;
                }
            }
            if let Some(noun) = self.noun {
                if line.id().noun_num() != noun {
                    continue;
                }
            }
            if let Some(condition) = self.condition {
                if line.id().condition_num() != condition {
                    continue;
                }
            }
            if let Some(sequence) = self.sequence {
                if line.id().sequence_num() != sequence {
                    continue;
                }
            }
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
        }
        Ok(())
    }
}

/// Checks message data, building a "book" and printing statistics and validation errors.
#[derive(Parser)]
struct CheckMessages {
    /// Path to a book file.
    book_path: PathBuf,
}

impl CheckMessages {
    fn run(&self) -> anyhow::Result<()> {
        let book: Book = scidev_book::file_format::deserialize_book(
            &mut serde_json::Deserializer::from_reader(std::fs::File::open(&self.book_path)?),
        )?;

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
            eprintln!("Room {}:", room.name().unwrap_or("*NO NAME*"));
            eprintln!("  Num Conditions: {}", room.conditions().count());
        }
        Ok(())
    }
}

/// Prints a list of all unique talker IDs found in the game messages.
#[derive(Parser)]
struct PrintTalkers {
    /// Path to the game's root directory.
    #[clap(index = 1)]
    root_dir: PathBuf,
}

impl PrintTalkers {
    fn run(&self) -> anyhow::Result<()> {
        let resource_set = open_game_resources(&self.root_dir)?;
        let mut talkers = BTreeSet::new();
        for res in resource_set.resources_of_type(ResourceType::Message) {
            let msg_resources = parse_message_resource(&res.load_data()?)?;
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

/// The specific message command to execute.
#[derive(Subcommand)]
enum MessageCommand {
    #[clap(about = "Prints messages from the game, with optional filters.")]
    Print(PrintMessages),
    #[clap(
        about = "Checks message data, building a \"book\" and printing statistics and validation errors."
    )]
    Check(CheckMessages),
    #[clap(
        name = "print-talkers",
        about = "Prints a list of all unique talker IDs found in the game messages."
    )]
    PrintTalkers(PrintTalkers),
}

/// Commands for working with game messages.
#[derive(Parser)]
pub struct Messages {
    /// The specific message command to execute.
    #[clap(subcommand)]
    msg_cmd: MessageCommand,
}

impl Messages {
    pub fn run(&self) -> anyhow::Result<()> {
        match &self.msg_cmd {
            MessageCommand::Print(cmd) => cmd.run()?,
            MessageCommand::Check(cmd) => cmd.run()?,
            MessageCommand::PrintTalkers(cmd) => cmd.run()?,
        }
        Ok(())
    }
}
