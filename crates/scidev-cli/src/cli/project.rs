use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::cmds::project::init_project;

#[derive(Parser)]
struct InitCommand {
    #[clap(index = 1)]
    path: Option<String>,
}

impl InitCommand {
    fn run(&self) -> anyhow::Result<()> {
        let target_dir: PathBuf = self.path.as_deref().unwrap_or(".").into();
        init_project(&target_dir)?;
        Ok(())
    }
}

#[derive(Subcommand)]
enum SubCommand {
    Init(InitCommand),
}

#[derive(Parser)]
pub(super) struct Cmd {
    #[clap(subcommand)]
    sub_command: SubCommand,
}

impl Cmd {
    pub(super) fn run(&self) -> anyhow::Result<()> {
        match &self.sub_command {
            SubCommand::Init(init) => init.run(),
        }
    }
}
