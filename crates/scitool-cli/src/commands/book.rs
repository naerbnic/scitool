use std::path::Path;

use scidev_book::{builder::BookBuilder, config::BookConfig};
use scidev_resources::{
    ResourceType, file::open_game_resources, types::msg::parse_message_resource,
};

pub fn export_book(
    config_path: &Path,
    game_path: &Path,
    output: impl std::io::Write,
) -> anyhow::Result<()> {
    let config = if config_path.exists() {
        let config: BookConfig = serde_yml::from_reader(std::fs::File::open(config_path)?)?;
        config
    } else {
        BookConfig::default()
    };
    let resource_set = open_game_resources(game_path)?;
    let mut builder = BookBuilder::new(config)?;

    // Extra testing for building a conversation.

    for res in resource_set.resources_of_type(ResourceType::Message) {
        let msg_resources = parse_message_resource(&res.load_data()?)?;
        for (msg_id, record) in msg_resources.messages() {
            builder.add_message(res.id().resource_num(), msg_id, record)?;
        }
    }
    let book = builder.build()?;

    scidev_book::file_format::serialize_book(&book, &mut serde_json::Serializer::new(output))?;

    Ok(())
}

pub fn validate_book(book_path: &Path) -> anyhow::Result<()> {
    let book = scidev_book::file_format::deserialize_book(
        &mut serde_json::Deserializer::from_reader(std::fs::File::open(book_path)?),
    )?;

    eprintln!(
        "Book loaded successfully with {} entries.",
        book.lines().count()
    );

    Ok(())
}

pub fn export_schema(pretty: bool) {
    let json_schema = scidev_book::file_format::json_schema(pretty);
    println!("{json_schema}");
}
