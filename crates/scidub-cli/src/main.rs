use clap::Parser as _;

mod cli;
mod commands;
mod data;
mod dirs;
mod project;

fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    cli.run()
}
