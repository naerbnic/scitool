use std::path::PathBuf;

use clap::{Parser, Subcommand};
use util::data_reader::IoDataReader;

mod res;
mod util;

#[derive(Parser)]
struct ListResources {
    #[clap(short, long)]
    res_map: PathBuf,
}

impl ListResources {
    fn run(&self) {
        let map_file = std::fs::File::open(&self.res_map).unwrap();
        let mut data_reader = IoDataReader::new(map_file);
        let resource_locations =
            res::mapfile::ResourceLocations::read_from(&mut data_reader).unwrap();
        eprintln!("{:#?}", resource_locations.type_ids().collect::<Vec<_>>());
    }
}

#[derive(Subcommand)]
enum ResourceCommand {
    #[clap(name = "list")]
    List(ListResources),
}

impl ResourceCommand {
    fn run(&self) {
        match self {
            ResourceCommand::List(list) => list.run(),
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
