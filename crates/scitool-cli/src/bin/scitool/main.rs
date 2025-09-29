use clap::Parser;

mod cli;

fn main() -> anyhow::Result<()> {
    let args = cli::Cli::parse();
    args.run()?;
    Ok(())
}
