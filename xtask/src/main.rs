//! A utility for setting up more complex build tasks.

cfg_select! {
    target_os = "macos" => {
        #[path = "plat_macos.rs"]
        mod plat;
    }
    _ => {
        #[path = "plat_default.rs"]
        mod plat;
    }
}

use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

use clap::{CommandFactory as _, Parser as _};
use sha2::Digest;

#[derive(clap::Parser)]
struct Env {
    #[arg(trailing_var_arg = true)]
    cmd_args: Vec<String>,
}

#[derive(clap::Subcommand)]
enum Command {
    Setup,
    Env(Env),
}

#[derive(clap::Parser)]
#[command(bin_name = "cargo x")]
struct Cli {
    #[clap(subcommand)]
    command: Command,
}

fn hash_directory(path: &Path, length: usize) -> String {
    let mut hasher = sha2::Sha256::default();
    hasher.update(path.as_os_str().as_encoded_bytes());
    let mut encoded = hex::encode(hasher.finalize());
    encoded.truncate(length);
    encoded
}

fn main() -> anyhow::Result<ExitCode> {
    let cli = Cli::parse();
    let Some(manifest_path) = std::env::var_os("CARGO_MANIFEST_DIR").map(PathBuf::from) else {
        Cli::command()
            .error(
                clap::error::ErrorKind::DisplayHelp,
                "Could not determine CARGO_MANIFEST_DIR, Run using `cargo x ...`",
            )
            .exit();
    };
    let Some(workspace_path) = manifest_path.parent() else {
        Cli::command()
            .error(
                clap::error::ErrorKind::DisplayHelp,
                "Cargo.toml is at the root of the workspace",
            )
            .exit();
    };

    let app_name = format!("xtask-{}", hash_directory(workspace_path, 8));

    let base_dirs = directories::ProjectDirs::from("", "scidev", &app_name)
        .ok_or_else(|| anyhow::anyhow!("Could not determine project directories"))?;

    match cli.command {
        Command::Setup => plat::run_setup(&base_dirs, workspace_path)?,
        Command::Env(env) => {
            let Some((cmd, args)) = env.cmd_args.split_first() else {
                Cli::command()
                    .error(
                        clap::error::ErrorKind::TooFewValues,
                        "Must provide at least one command to run in the environment.",
                    )
                    .exit();
            };

            return plat::run_env(workspace_path, cmd, args);
        }
    }

    Ok(ExitCode::SUCCESS)
}
