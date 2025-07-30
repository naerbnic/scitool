#![expect(
    clippy::doc_markdown,
    reason = "This module's docs are user-facing and should be descriptive"
)]
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use scidev_resources::{ResourceId, ResourceType, file::open_game_resources};
use scidev_utils::data_writer::{DataWriter, IoDataWriter};

mod book;
mod generate;
mod msg;
mod script;

/// Lists resources in the game.
#[derive(Parser)]
struct ListResources {
    /// Path to the game's root directory.
    #[clap(index = 1)]
    root_dir: PathBuf,
    #[expect(clippy::doc_markdown, reason = "This is a user-directed help")]
    /// Filter by resource type (e.g., Script, Heap, View, Pic, Sound, Message, Font, Cursor, Patch, AudioPath, Vocab, Palette, Wave, Audio, Sync).
    #[clap(long = "type", short = 't')]
    res_type: Option<ResourceType>,
}

impl ListResources {
    fn run(&self) -> anyhow::Result<()> {
        let resource_dir_files = open_game_resources(&self.root_dir)?;
        for id in resource_dir_files.resource_ids() {
            if let Some(res_type) = self.res_type {
                if id.type_id() != res_type {
                    continue;
                }
            }
            println!("{id:?}");
        }
        Ok(())
    }
}

/// Extracts a resource and saves it as a patch file. Supported types are Script (SCR) and Heap (HEP).
#[derive(Parser)]
struct ExtractResourceAsPatch {
    /// Path to the game's root directory.
    #[clap(index = 1)]
    root_dir: PathBuf,
    /// The type of the resource to extract (e.g., Script, Heap).
    #[clap(index = 2)]
    resource_type: ResourceType,
    /// The ID of the resource to extract.
    #[clap(index = 3)]
    resource_id: u16,
    /// If set, prints what would be done without actually writing files.
    #[clap(short = 'n', long, default_value = "false")]
    dry_run: bool,
    /// Directory to save the output file. Defaults to <root_dir>.
    #[clap(short = 'o', long)]
    output_dir: Option<PathBuf>,
}

impl ExtractResourceAsPatch {
    fn run(&self) -> anyhow::Result<()> {
        let resource_set = open_game_resources(&self.root_dir)?;
        let resource_id = ResourceId::new(self.resource_type, self.resource_id);
        let contents = resource_set
            .get_resource(&resource_id)
            .ok_or_else(|| anyhow::anyhow!("Resource not found: {:?}", resource_id))?;
        let ext = match self.resource_type {
            ResourceType::Script => "SCR",
            ResourceType::Heap => "HEP",
            _ => {
                anyhow::bail!("Unsupported resource type");
            }
        };

        let out_root = self.output_dir.as_ref().unwrap_or(&self.root_dir);

        let filename = out_root.join(format!("{0}.{1}", self.resource_id, ext));
        if self.dry_run {
            eprintln!(
                "DRY_RUN: Writing resource {restype:?}:{resid} to {filename}",
                restype = self.resource_type,
                resid = self.resource_id,
                filename = filename.display()
            );
        } else {
            eprintln!(
                "Writing resource {restype:?}:{resid} to {filename}",
                restype = self.resource_type,
                resid = self.resource_id,
                filename = filename.display()
            );
            {
                let mut patch_file = IoDataWriter::new(
                    std::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(self.root_dir.join(filename))?,
                );

                patch_file.write_u8(self.resource_type.into())?;
                patch_file.write_u8(0)?; // Header Size
                patch_file.write_block(&contents.load_data()?)?;
            }
        }

        Ok(())
    }
}

/// Dumps the hexadecimal content of a resource.
#[derive(Parser)]
struct DumpResource {
    /// Path to the game's root directory.
    #[clap(index = 1)]
    root_dir: PathBuf,
    /// The type of the resource to dump.
    #[clap(index = 2)]
    resource_type: ResourceType,
    /// The ID of the resource to dump.
    #[clap(index = 3)]
    resource_id: u16,
}

impl DumpResource {
    fn run(&self) -> anyhow::Result<()> {
        let resource_set = open_game_resources(&self.root_dir)?;
        let resource_id = ResourceId::new(self.resource_type, self.resource_id);
        let res = resource_set
            .get_resource(&resource_id)
            .ok_or_else(|| anyhow::anyhow!("Resource not found: {:?}", resource_id))?;
        let data = res.load_data()?;
        scidev_utils::debug::hex_dump(&data, 0);
        Ok(())
    }
}

/// The specific resource command to execute.
#[derive(Subcommand)]
enum ResourceCommand {
    #[clap(name = "list", about = "Lists resources in the game.")]
    List(ListResources),
    #[clap(
        about = "Extracts a resource and saves it as a patch file. Supported types are Script (SCR) and Heap (HEP)."
    )]
    ExtractAsPatch(ExtractResourceAsPatch),
    #[clap(about = "Dumps the hexadecimal content of a resource.")]
    Dump(DumpResource),
}

impl ResourceCommand {
    fn run(&self) -> anyhow::Result<()> {
        match self {
            ResourceCommand::List(list) => list.run()?,
            ResourceCommand::ExtractAsPatch(extract) => extract.run()?,
            ResourceCommand::Dump(dump) => dump.run()?,
        }
        Ok(())
    }
}

/// Commands for working with game resources.
#[derive(Parser)]
struct Resource {
    /// The specific resource command to execute.
    #[clap(subcommand)]
    res_cmd: ResourceCommand,
}

impl Resource {
    fn run(&self) -> anyhow::Result<()> {
        self.res_cmd.run()
    }
}

/// The category of command to run.
#[derive(Subcommand)]
enum Category {
    #[clap(name = "res", about = "Commands for working with game resources.")]
    Resource(Resource),
    #[clap(name = "msg", about = "Commands for working with game messages.")]
    Message(msg::Messages),
    #[clap(
        name = "gen",
        about = "Commands for generating various outputs from game data."
    )]
    Generate(generate::Generate),
    #[clap(name = "script", about = "Commands for working with game scripts.")]
    Script(script::Script),
    #[clap(name = "book", about = "Commands for working with game books.")]
    Book(book::BookCommand),
}

impl Category {
    fn run(&self) -> anyhow::Result<()> {
        match self {
            Category::Resource(res) => res.run(),
            Category::Message(msg) => msg.run(),
            Category::Generate(generate) => generate.run(),
            Category::Script(script) => script.run(),
            Category::Book(book) => book.run(),
        }
    }
}

/// A command line tool for working with Sierra adventure games written in the SCI engine.
#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
pub struct Cli {
    /// The category of command to run.
    #[clap(subcommand)]
    category: Category,
}

impl Cli {
    pub fn run(&self) -> anyhow::Result<()> {
        self.category.run()
    }
}
