use crate::cmds::Cli;
use clap::Parser;

mod cmds;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Cli::parse().run().await?;
    Ok(())
}
