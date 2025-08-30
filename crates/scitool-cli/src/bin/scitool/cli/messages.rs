use std::path::PathBuf;

use clap::{Parser, Subcommand};

use scitool_cli::commands::messages::print_talkers;

/// Prints a list of all unique talker IDs found in the game messages.
#[derive(Parser)]
struct PrintTalkers {
    /// Path to the game's root directory.
    #[clap(index = 1)]
    root_dir: PathBuf,
}

impl PrintTalkers {
    fn run(&self) -> anyhow::Result<()> {
        print_talkers(&self.root_dir, std::io::stdout().lock())
    }
}

/// The specific message command to execute.
#[derive(Subcommand)]
enum MessageCommand {
    /// Prints a list of all unique talker IDs found in the game messages.
    PrintTalkers(PrintTalkers),
}

/// Commands for working with game messages.
#[derive(Parser)]
pub(super) struct Messages {
    /// The specific message command to execute.
    #[clap(subcommand)]
    msg_cmd: MessageCommand,
}

impl Messages {
    pub(super) fn run(&self) -> anyhow::Result<()> {
        match &self.msg_cmd {
            MessageCommand::PrintTalkers(cmd) => cmd.run()?,
        }
        Ok(())
    }
}
