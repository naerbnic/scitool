//! A CLI for working with SCI games.
//!
//! This includes both low-level tools for primitive operations on game resources
//! and higher-level project management tools.

mod cli;
mod cmds;

use crate::cli::Cli;
use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Cli::parse().run().await?;
    Ok(())
}
