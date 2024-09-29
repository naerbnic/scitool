use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::{
    book::{builder::BookBuilder, config::BookConfig, Book},
    gen::{
        doc::{Document, DocumentBuilder},
        html::generate_html,
    },
    res::{file::open_game_resources, msg::parse_message_resource, ResourceType},
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

    for (id, res) in resource_set.resources_of_type(ResourceType::Message) {
        let msg_resources = parse_message_resource(res.open()?)?;
        for (msg_id, record) in msg_resources.messages() {
            builder.add_message(id.resource_num, msg_id, record)?;
        }
    }
    Ok(builder.build()?)
}

fn generate_document(book: &Book) -> anyhow::Result<Document> {
    let mut doc = DocumentBuilder::new("SQ5: The Game: The Script");
    for room in book.rooms() {
        let mut room_section = doc.add_chapter(room.name()).into_section_builder();
        for noun in room.nouns() {
            let mut noun_section = room_section
                .add_subsection(
                    noun.desc()
                        .map(ToOwned::to_owned)
                        .unwrap_or_else(|| format!("{:?}", noun.id())),
                )
                .into_section_builder();
            for conversion in noun.conversations() {
                let mut conv_section =
                    noun_section.add_subsection(format!("{:?}", conversion.id()));
                let mut content = conv_section.add_content();
                let mut dialogue = content.add_dialogue();
                for line in conversion.lines() {
                    dialogue.add_line(line.role().short_name(), line.text())
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
