use clap::Parser;

mod book;
mod cli;
mod gen;
mod output;

fn main() -> anyhow::Result<()> {
    let args = cli::Cli::parse();
    args.run()?;
    Ok(())
}
