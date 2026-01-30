//! A low-level command-line tool for reading and constructing SCI games.

use crate::cmds::Cli;
use clap::Parser;

mod cmds;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Cli::parse().run().await?;
    Ok(())
}
