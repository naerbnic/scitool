use clap::Parser;

mod cli;
mod res;
mod util;
mod output;

fn main() -> anyhow::Result<()> {
    let args = cli::Cli::parse();
    args.run()?;
    Ok(())
}
