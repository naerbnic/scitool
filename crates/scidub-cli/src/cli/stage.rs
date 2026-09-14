use clap::Parser;

use crate::cli::GlobalConfigArgs;

/// Build the game, and stage it in the game's build directory.
///
/// Game files in the manifest will be copied from the existing configured
/// game files directory.
#[derive(Parser, Debug)]
pub(super) struct Stage {
    #[command(flatten)]
    env: GlobalConfigArgs,

    /// The name of the game target to stage.
    #[arg(index = 1, required = true)]
    target: Option<String>,
}

impl Stage {
    pub(super) fn run(self) -> anyhow::Result<()> {
        Ok(())
    }
}
