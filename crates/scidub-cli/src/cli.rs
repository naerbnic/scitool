use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::project::Project;

mod build;
mod check_distribution;
mod game_data;
mod init;
mod script;

/// A utility for managing and building an SCI fan-dub project.
#[derive(Debug, Parser)]
#[clap(version, about)]
pub(crate) struct Cli {
    #[command(subcommand)]
    command: Command,
}

impl Cli {
    pub(crate) fn run(self) -> anyhow::Result<()> {
        self.command.run()
    }
}

#[derive(Debug, Subcommand)]
enum Command {
    Init(init::Init),
    Script(script::ScriptSubCommand),
    Build(build::Build),
    GameData(game_data::GameData),
    #[clap(hide = true)]
    CheckDistribution(check_distribution::CheckDistribution),
}

impl Command {
    fn run(self) -> anyhow::Result<()> {
        match self {
            Self::Init(init) => init.run(),
            Self::Script(s) => s.run(),
            Self::Build(build) => build.run(),
            Self::GameData(game_data) => game_data.run(),
            Self::CheckDistribution(c) => c.run(),
        }
    }
}

/// Common flag arguments for all project-based commands.
#[derive(Debug, clap::Args)]
struct GlobalConfigArgs {
    /// Provides an explicit root for the project.
    #[arg(long)]
    project_root: Option<PathBuf>,
}

impl GlobalConfigArgs {
    /// Load a project from the command line flags and/or current process
    /// environment.
    fn load_project(self) -> anyhow::Result<Project> {
        let project = if let Some(project_root) = self.project_root {
            Project::new(project_root)
        } else {
            Project::new_from_path(&std::env::current_dir()?)?
        };

        Ok(project)
    }
}
