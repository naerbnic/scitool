#![recursion_limit = "1024"]

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use res::{ResourceId, ResourceType};
use util::{
    data_reader::IoDataReader,
    data_writer::{DataWriter, IoDataWriter},
};

mod res;
mod util;

#[derive(Parser)]
struct ListResources {
    #[clap(index = 1)]
    root_dir: PathBuf,
}

impl ListResources {
    fn run(&self) {
        let map_file = std::fs::File::open(self.root_dir.join("RESOURCE.MAP")).unwrap();
        let data_file = std::fs::File::open(self.root_dir.join("RESOURCE.000")).unwrap();
        let mut data_reader = IoDataReader::new(map_file);
        let resource_locations =
            res::mapfile::ResourceLocations::read_from(&mut data_reader).unwrap();
        let mut data_file = res::datafile::DataFile::new(data_file);
        for location in resource_locations.locations() {
            let header = data_file.read_raw_header(&location).unwrap();
            println!("{:?}, {:?}", location, header);
        }
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
        let map_file = std::fs::File::open(self.root_dir.join("RESOURCE.MAP")).unwrap();
        let data_file = std::fs::File::open(self.root_dir.join("RESOURCE.000")).unwrap();
        let mut data_reader = IoDataReader::new(map_file);
        let resource_locations =
            res::mapfile::ResourceLocations::read_from(&mut data_reader).unwrap();
        let mut data_file = res::datafile::DataFile::new(data_file);

        let location = resource_locations
            .get_location(&ResourceId::new(self.resource_type, self.resource_id))
            .unwrap();
        let contents = data_file.read_contents(&location)?;
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
            patch_file.write_all(contents.data())?;
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
    fn run(&self) {
        match self {
            ResourceCommand::List(list) => list.run(),
            ResourceCommand::ExtractAsPatch(extract) => extract.run().unwrap(),
        }
    }
}

#[derive(Parser)]
struct Resource {
    #[clap(subcommand)]
    res_cmd: ResourceCommand,
}

impl Resource {
    fn run(&self) {
        self.res_cmd.run();
    }
}

#[derive(Subcommand)]
enum Category {
    #[clap(name = "res")]
    Resource(Resource),
}

impl Category {
    fn run(&self) {
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
    fn run(&self) {
        self.category.run();
    }
}

fn main() -> anyhow::Result<()> {
    let args = Cli::parse();
    args.run();
    Ok(())
}
