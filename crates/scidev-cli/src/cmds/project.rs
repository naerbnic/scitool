use std::path::PathBuf;

use clap::{Parser, Subcommand};
use sciproj::project::Project;

#[derive(Parser)]
struct InitCommand {
    #[clap(index = 1)]
    path: Option<String>,
}

impl InitCommand {
    fn run(&self) -> anyhow::Result<()> {
        let target_dir: PathBuf = self.path.as_deref().unwrap_or(".").into();
        let target_dir = target_dir.canonicalize()?;

        // Some initial checks to sanity check the target directory

        println!("Initializing new project in {}", target_dir.display());
        Project::create_at(&target_dir)?;
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
