use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
struct GenerateHeaders {
    #[arg(short = 'd')]
    game_dir: PathBuf,
    #[arg(short = 'o', long, default_value = ".")]
    out_dir: PathBuf,
    #[arg(short = 's', long, default_value = "selectors.sh")]
    selectors_path: PathBuf,
    #[arg(short = 'c', long, default_value = "classdef.sh")]
    classdef_path: PathBuf,
}

impl GenerateHeaders {
    pub fn run(&self) -> anyhow::Result<()> {
        let exports = sci_header_gen::SciScriptExports::read_from_resources(&self.game_dir)?;

        let selectors_file = std::fs::File::create(self.out_dir.join(&self.selectors_path))?;
        exports.write_selector_header_to(std::io::BufWriter::new(selectors_file))?;

        let classdef_file = std::fs::File::create(self.out_dir.join(&self.classdef_path))?;
        exports.write_classdef_header_to(std::io::BufWriter::new(classdef_file))?;

        Ok(())
    }
}

#[derive(Subcommand)]
enum ScriptCommand {
    #[clap(name = "gen-headers")]
    GenerateHeaders(GenerateHeaders),
}

impl ScriptCommand {
    fn run(&self) -> anyhow::Result<()> {
        match self {
            ScriptCommand::GenerateHeaders(gen_headers) => gen_headers.run()?,
        }
        Ok(())
    }
}

#[derive(Parser)]
pub struct Script {
    #[clap(subcommand)]
    command: ScriptCommand,
}

impl Script {
    pub fn run(&self) -> anyhow::Result<()> {
        self.command.run()
    }
}
