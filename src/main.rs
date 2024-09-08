use clap::Parser;

mod cli;
mod msg;
mod res;
mod util;

fn main() -> anyhow::Result<()> {
    let args = cli::Cli::parse();
    args.run()?;
    Ok(())
}
