use clap::Parser;

mod book;
mod cli;
mod generate;
mod output;

fn main() -> anyhow::Result<()> {
    let args = cli::Cli::parse();
    args.run()?;
    Ok(())
}
