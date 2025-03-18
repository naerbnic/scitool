use std::path::PathBuf;

use clap::{Parser, Subcommand};
use sci_resources::{ResourceId, ResourceType, file::open_game_resources};
use sci_utils::data_writer::{DataWriter, IoDataWriter};

mod args;
mod generate;
mod msg;
mod script;

#[derive(Parser)]
struct ListResources {
    #[clap(index = 1)]
    root_dir: PathBuf,
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
            println!("{:?}", id);
        }
        Ok(())
    }
}

#[derive(Parser)]
struct ExtractResourceAsPatch {
    #[clap(index = 1)]
    root_dir: PathBuf,
    #[clap(index = 2)]
    resource_type: ResourceType,
    #[clap(index = 3)]
    resource_id: u16,
    #[clap(short = 'n', long, default_value = "false")]
    dry_run: bool,
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
                "DRY_RUN: Writing resource {restype:?}:{resid} to {filename:?}",
                restype = self.resource_type,
                resid = self.resource_id,
                filename = filename
            );
        } else {
            eprintln!(
                "Writing resource {restype:?}:{resid} to {filename:?}",
                restype = self.resource_type,
                resid = self.resource_id,
                filename = filename
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

#[derive(Parser)]
struct DumpResource {
    #[clap(index = 1)]
    root_dir: PathBuf,
    #[clap(index = 2)]
    resource_type: ResourceType,
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
        sci_utils::debug::hex_dump(&data, 0);
        Ok(())
    }
}

#[derive(Subcommand)]
enum ResourceCommand {
    #[clap(name = "list")]
    List(ListResources),
    ExtractAsPatch(ExtractResourceAsPatch),
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

#[derive(Parser)]
struct Resource {
    #[clap(subcommand)]
    res_cmd: ResourceCommand,
}

impl Resource {
    fn run(&self) -> anyhow::Result<()> {
        self.res_cmd.run()
    }
}

#[derive(Subcommand)]
enum Category {
    #[clap(name = "res")]
    Resource(Resource),
    #[clap(name = "msg")]
    Message(msg::Messages),
    #[clap(name = "gen")]
    Generate(generate::Generate),
    #[clap(name = "script")]
    Script(script::Script),
}

impl Category {
    fn run(&self) -> anyhow::Result<()> {
        match self {
            Category::Resource(res) => res.run(),
            Category::Message(msg) => msg.run(),
            Category::Generate(generate) => generate.run(),
            Category::Script(script) => script.run(),
        }
    }
}

#[derive(Parser)]
pub struct Cli {
    #[clap(subcommand)]
    category: Category,
}

impl Cli {
    pub fn run(&self) -> anyhow::Result<()> {
        self.category.run()
    }
}
