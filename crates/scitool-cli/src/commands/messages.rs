use std::{collections::BTreeSet, path::Path};

use scidev_book::Book;
use scidev_resources::{
    ResourceType, file::open_game_resources, types::msg::parse_message_resource,
};

pub struct LineFilter {
    /// Filter by talker ID.
    talker: Option<u8>,
    /// Filter by room ID.
    room: Option<u16>,
    /// Filter by verb ID.
    verb: Option<u8>,
    /// Filter by noun ID.
    noun: Option<u8>,
    /// Filter by condition ID.
    condition: Option<u8>,
    /// Filter by sequence ID.
    sequence: Option<u8>,
}

impl LineFilter {
    /// Create a new filter with all fields set to `None`.
    #[must_use]
    pub fn new(
        talker: Option<u8>,
        room: Option<u16>,
        verb: Option<u8>,
        noun: Option<u8>,
        condition: Option<u8>,
        sequence: Option<u8>,
    ) -> Self {
        Self {
            talker,
            room,
            verb,
            noun,
            condition,
            sequence,
        }
    }
}

pub fn for_each_line(
    book_path: &Path,
    filter: &LineFilter,
    mut body: impl for<'a> FnMut(&'a scidev_book::Line<'a>) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let book: Book = scidev_book::file_format::deserialize_book(
        &mut serde_json::Deserializer::from_reader(std::fs::File::open(book_path)?),
    )?;
    for line in book.lines() {
        if let Some(room) = filter.room {
            if line.id().room_num() != room {
                continue;
            }
        }
        if let Some(talker) = filter.talker {
            if line.talker_num() != talker {
                continue;
            }
        }
        if let Some(verb) = filter.verb {
            if line.id().verb_num() != verb {
                continue;
            }
        }
        if let Some(noun) = filter.noun {
            if line.id().noun_num() != noun {
                continue;
            }
        }
        if let Some(condition) = filter.condition {
            if line.id().condition_num() != condition {
                continue;
            }
        }
        if let Some(sequence) = filter.sequence {
            if line.id().sequence_num() != sequence {
                continue;
            }
        }

        body(&line)?;
    }
    Ok(())
}

pub fn check_messages(book_path: &Path, mut output: impl std::io::Write) -> anyhow::Result<()> {
    let book: Book = scidev_book::file_format::deserialize_book(
        &mut serde_json::Deserializer::from_reader(std::fs::File::open(book_path)?),
    )?;

    writeln!(output, "Num rooms: {}", book.rooms().count())?;
    writeln!(output, "Num nouns: {}", book.nouns().count())?;
    writeln!(
        output,
        "Num conversations: {}",
        book.conversations().count()
    )?;
    writeln!(
        output,
        "Num multi-line conversations: {}",
        book.conversations()
            .filter(|c| c.lines().count() > 1)
            .count()
    )?;
    writeln!(output, "Num lines: {}", book.lines().count())?;
    writeln!(
        output,
        "Num empty lines: {}",
        book.lines().filter(|line| line.text().is_empty()).count()
    )?;

    for conversation in book.conversations() {
        if let Err(e) = conversation.validate_complete() {
            writeln!(output, "Conversation {:?}: {}", conversation.id(), e)?;
        }
    }

    for room in book.rooms() {
        writeln!(output, "Room {}:", room.name().unwrap_or("*NO NAME*"))?;
        writeln!(output, "  Num Conditions: {}", room.conditions().count())?;
    }
    Ok(())
}

pub fn print_talkers(game_dir: &Path, mut output: impl std::io::Write) -> anyhow::Result<()> {
    let resource_set = open_game_resources(game_dir)?;
    let mut talkers = BTreeSet::new();
    for res in resource_set.resources_of_type(ResourceType::Message) {
        let msg_resources = parse_message_resource(&res.load_data()?)?;
        for (_, record) in msg_resources.messages() {
            talkers.insert(record.talker());
        }
    }
    write!(output, "Talkers:")?;
    write!(
        output,
        "  {}",
        talkers
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    )?;
    Ok(())
}
