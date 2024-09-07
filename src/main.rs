#![expect(dead_code)]

use std::{fs::File, io, path::PathBuf};

use clap::{Parser, Subcommand};
use res::{
    datafile::{Contents, DataFile, RawContents},
    mapfile::{ResourceLocation, ResourceLocations},
    ResourceId, ResourceType,
};
use util::{
    block::{Block, BlockReader},
    data_writer::{DataWriter, IoDataWriter},
};

mod res;
mod util;

struct ResourceDirFiles {
    resource_locations: ResourceLocations,
    data_file: DataFile,
}

impl ResourceDirFiles {
    fn open(root_dir: &PathBuf) -> io::Result<Self> {
        let map_file = Block::from_reader(File::open(root_dir.join("RESOURCE.MAP"))?)?;
        let data_file = DataFile::new(Block::from_reader(File::open(
            root_dir.join("RESOURCE.000"),
        )?)?);
        let resource_locations =
            res::mapfile::ResourceLocations::read_from(BlockReader::new(map_file))?;
        Ok(ResourceDirFiles {
            resource_locations,
            data_file,
        })
    }

    pub fn read_raw_contents(
        &self,
    ) -> impl Iterator<Item = io::Result<(ResourceLocation, RawContents)>> + '_ {
        self.resource_locations.locations().map(move |location| {
            let raw_contents = self.data_file.read_raw_contents(&location)?;
            Ok((location, raw_contents))
        })
    }

    pub fn read_resource(&self, res_type: ResourceType, res_num: u16) -> io::Result<Contents> {
        let location = self
            .resource_locations
            .get_location(&ResourceId::new(res_type, res_num))
            .unwrap();
        Ok(self.data_file.read_contents(&location)?)
    }
}

#[derive(Parser)]
struct ListResources {
    #[clap(index = 1)]
    root_dir: PathBuf,
}

impl ListResources {
    fn run(&self) -> anyhow::Result<()> {
        let resource_dir_files = ResourceDirFiles::open(&self.root_dir)?;
        for item in resource_dir_files.read_raw_contents() {
            let (location, header) = item?;
            println!("{:?}, {:?}", location, header);
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
}

impl ExtractResourceAsPatch {
    fn run(&self) -> anyhow::Result<()> {
        let resource_dir_files = ResourceDirFiles::open(&self.root_dir)?;
        let contents = resource_dir_files.read_resource(self.resource_type, self.resource_id)?;
        let ext = match self.resource_type {
            ResourceType::Script => "SCR",
            ResourceType::Heap => "HEP",
            _ => {
                anyhow::bail!("Unsupported resource type");
            }
        };

        let filename = format!("{0}.{1}", self.resource_id, ext);
        eprintln!(
            "Writing resource {restype:?}:{resid} to {filename}",
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
            patch_file.write_block(&contents.data())?;
        }

        Ok(())
    }
}

#[derive(Subcommand)]
enum ResourceCommand {
    #[clap(name = "list")]
    List(ListResources),
    ExtractAsPatch(ExtractResourceAsPatch),
}

impl ResourceCommand {
    fn run(&self) -> anyhow::Result<()> {
        match self {
            ResourceCommand::List(list) => list.run()?,
            ResourceCommand::ExtractAsPatch(extract) => extract.run()?,
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
}

impl Category {
    fn run(&self) -> anyhow::Result<()> {
        match self {
            Category::Resource(res) => res.run(),
        }
    }
}

#[derive(Parser)]
struct Cli {
    #[clap(subcommand)]
    category: Category,
}

impl Cli {
    fn run(&self) -> anyhow::Result<()> {
        self.category.run()
    }
}

fn main() -> anyhow::Result<()> {
    let args = Cli::parse();
    args.run()?;
    Ok(())
}
