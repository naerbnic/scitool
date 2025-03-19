use clap::Parser;

fn main() -> anyhow::Result<()> {
    let args = scitool_cli::cli::Cli::parse();
    args.run()?;
    Ok(())
}
