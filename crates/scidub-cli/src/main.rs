use clap::Parser as _;

mod cli;
mod commands;
mod data;

fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    cli.run()
}
