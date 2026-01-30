use sciproj::book::{Book, Line, builder::BookBuilder, config::BookConfig, file_format};

use std::path::Path;

use scidev::resources::{ResourceSet, ResourceType, types::msg::parse_message_resource};

pub(crate) fn export_book(
    config_path: &Path,
    game_path: &Path,
    output: impl std::io::Write,
) -> anyhow::Result<()> {
    let config = if config_path.exists() {
        let config: BookConfig = serde_norway::from_reader(std::fs::File::open(config_path)?)?;
        config
    } else {
        BookConfig::default()
    };
    let resource_set = ResourceSet::from_root_dir(game_path)?;
    let mut builder = BookBuilder::new(config)?;

    // Extra testing for building a conversation.

    for res in resource_set.resources_of_type(ResourceType::Message) {
        let msg_resources = parse_message_resource(&res.data().open_mem(..)?)?;
        for (msg_id, record) in msg_resources.messages() {
            builder.add_message(res.id().resource_num(), msg_id, record)?;
        }
    }
    let book = builder.build()?;

    file_format::serialize_book(&book, &mut serde_json::Serializer::new(output))?;

    Ok(())
}

pub(crate) fn validate_book(book_path: &Path) -> anyhow::Result<()> {
    let book = file_format::deserialize_book(&mut serde_json::Deserializer::from_reader(
        std::fs::File::open(book_path)?,
    ))?;

    eprintln!(
        "Book loaded successfully with {} entries.",
        book.lines().count()
    );

    Ok(())
}

pub(crate) fn export_schema(pretty: bool) {
    let json_schema = file_format::json_schema(pretty);
    println!("{json_schema}");
}

pub(crate) struct LineFilter {
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
    pub(crate) fn new(
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

pub(crate) fn for_each_line(
    book_path: &Path,
    filter: &LineFilter,
    mut body: impl for<'a> FnMut(&'a Line<'a>) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let book: Book = file_format::deserialize_book(&mut serde_json::Deserializer::from_reader(
        std::fs::File::open(book_path)?,
    ))?;
    for line in book.lines() {
        if let Some(room) = filter.room
            && line.id().room_num() != room
        {
            continue;
        }
        if let Some(talker) = filter.talker
            && line.talker_num() != talker
        {
            continue;
        }
        if let Some(verb) = filter.verb
            && line.id().verb_num() != verb
        {
            continue;
        }
        if let Some(noun) = filter.noun
            && line.id().noun_num() != noun
        {
            continue;
        }
        if let Some(condition) = filter.condition
            && line.id().condition_num() != condition
        {
            continue;
        }
        if let Some(sequence) = filter.sequence
            && line.id().sequence_num() != sequence
        {
            continue;
        }

        body(&line)?;
    }
    Ok(())
}
