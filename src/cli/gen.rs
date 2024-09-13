use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
struct CommonArgs {
    root_dir: PathBuf,
}

#[derive(Parser)]
struct GenerateMaster {
    #[clap(flatten)]
    ctxt: CommonArgs,
}

impl GenerateMaster {
    fn run(&self) -> anyhow::Result<()> {
        println!("Generate Master: {:?}", self.ctxt.root_dir);
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
