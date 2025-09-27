use std::path::PathBuf;

use clap::{Parser, Subcommand};

use scitool_cli::commands::scripts::{dump_headers, generate_headers};

/// Generates script header files (`selectors.sh` and `classdef.sh`) from game resources.
#[derive(Parser)]
struct GenerateHeaders {
    /// Path to the game's root directory.
    #[arg(short = 'd', long)]
    game_dir: PathBuf,

    /// Directory to write the header files. Defaults to the current directory (`.`)
    #[arg(short = 'o', long, default_value = ".")]
    out_dir: PathBuf,

    /// Filename for the selectors header. Defaults to `selectors.sh`
    #[arg(short = 's', long, default_value = "selectors.sh")]
    selectors_path: PathBuf,

    /// Filename for the class definition header. Defaults to `classdef.sh`
    #[arg(short = 'c', long, default_value = "classdef.sh")]
    classdef_path: PathBuf,

    #[arg(short = 'n', long, default_value = "false")]
    dry_run: bool,
}

impl GenerateHeaders {
    pub(crate) async fn run(&self) -> anyhow::Result<()> {
        if self.dry_run {
            dump_headers(&self.game_dir, std::io::stdout()).await?;
        } else {
            generate_headers(
                &self.game_dir,
                &self.out_dir.join(&self.selectors_path),
                &self.out_dir.join(&self.classdef_path),
            )
            .await?;
        }
        Ok(())
    }
}

/// The specific script command to execute.
#[derive(Subcommand)]
enum ScriptCommand {
    #[clap(
        name = "gen-headers",
        about = "Generates script header files (`selectors.sh` and `classdef.sh`) from game resources."
    )]
    GenerateHeaders(GenerateHeaders),
}

impl ScriptCommand {
    async fn run(&self) -> anyhow::Result<()> {
        match self {
            ScriptCommand::GenerateHeaders(gen_headers) => gen_headers.run().await?,
        }
        Ok(())
    }
}

/// Commands for working with game scripts.
#[derive(Parser)]
pub(super) struct Script {
    /// The specific script command to execute.
    #[clap(subcommand)]
    command: ScriptCommand,
}

impl Script {
    pub(super) async fn run(&self) -> anyhow::Result<()> {
        self.command.run().await
    }
}
