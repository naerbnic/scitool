use std::path::PathBuf;

use clap::{Parser, Subcommand};
use sci_utils::buffer::Buffer;
use std::io::Write;

#[derive(Parser)]
#[command(about = "Dump a selectors file compatable with the sc compiler for the game.")]
struct DumpSelectorsFile {
    #[clap(index = 1)]
    root_dir: PathBuf,
    #[clap(short = 'o', long)]
    output: PathBuf,
}

impl DumpSelectorsFile {
    fn run(&self) -> anyhow::Result<()> {
        let resource_set = sci_resources::file::open_game_resources(&self.root_dir)?;
        let selector_table_resource = resource_set
            .get_resource(&sci_resources::ResourceId::new(
                sci_resources::ResourceType::Vocab,
                997,
            ))
            .ok_or_else(|| anyhow::anyhow!("Selector table not found"))?;
        let selector_table = sci_resources::types::selector_table::SelectorTable::load_from(
            selector_table_resource.load_data()?.narrow(),
        )?;

        // The output expects an S-Expression like this:
        //
        // (selectors
        //   selector-name <selector-id>
        //   ...)

        let mut selectors_file = std::io::BufWriter::new(std::fs::File::create(&self.output)?);
        // Note that we leave the next write location at the end of the line,
        // to write the correct closing paren.
        write!(selectors_file, "(selectors")?;

        for selector in selector_table.selectors() {
            write!(selectors_file, "\n  {} {}", selector.name(), selector.id())?;
        }
        writeln!(selectors_file, ")")?;
        selectors_file.flush()?;
        Ok(())
    }
}

#[derive(Subcommand)]
enum ScriptCommand {
    #[clap(name = "dump-selectors")]
    DumpSelectorsFile(DumpSelectorsFile),
}

impl ScriptCommand {
    fn run(&self) -> anyhow::Result<()> {
        match self {
            ScriptCommand::DumpSelectorsFile(dump) => dump.run()?,
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
