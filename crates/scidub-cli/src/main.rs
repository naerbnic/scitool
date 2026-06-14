use clap::Parser as _;

mod cli;
mod commands;
mod data;
mod dist_env;
mod project;

fn main() {
    let cli = cli::Cli::parse();
    if let Err(err) = cli.run() {
        eprintln!("{err:#}");
        std::process::exit(1);
    }
}
