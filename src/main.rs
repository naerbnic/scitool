use clap::Parser;

mod book;
mod cli;
mod gen;
mod output;
mod res;
mod util;

fn main() -> anyhow::Result<()> {
    let args = cli::Cli::parse();
    args.run()?;
    Ok(())
}
