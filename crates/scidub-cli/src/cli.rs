use std::{cell::OnceCell, path::PathBuf, rc::Rc};

use anyhow::Context;
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
#[derive(Debug, clap::Args, Clone)]
struct GlobalConfigArgs {
    /// Provides an explicit root for the project.
    #[arg(long)]
    project_root: Option<PathBuf>,

    #[arg(skip)]
    project: Rc<OnceCell<Project>>,
}

impl GlobalConfigArgs {
    /// Load a project from the command line flags and/or current process
    /// environment.
    fn load_project(&self) -> anyhow::Result<&Project> {
        if let Some(project) = self.project.get() {
            return Ok(project);
        }
        let project = if let Some(project_root) = &self.project_root {
            let project_root_buf = if project_root.is_absolute() {
                project_root.clone()
            } else {
                std::env::current_dir()?.join(project_root)
            };
            let metadata = project_root_buf.metadata().context(format!(
                "When looking for project root: {}",
                project_root.display()
            ))?;
            anyhow::ensure!(metadata.is_dir(), "Provided project is not a directory");
            Project::new(project_root_buf.clone())
        } else {
            Project::new_from_path(&std::env::current_dir()?)?
        };
        self.project.set(project).unwrap();
        Ok(self.project.get().unwrap())
    }
}
