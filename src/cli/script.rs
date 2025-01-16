use std::path::PathBuf;

use clap::{Parser, Subcommand};
use scitool_script_loader::{Class, ClassDeclSet, ScriptLoader, Species};
use std::io::Write;

use super::args::OutFilePath;

#[derive(Parser)]
#[command(about = "Dump a selectors file compatable with the sc compiler for the game.")]
struct DumpSelectorsFile {
    #[clap(short = 'd')]
    root_dir: PathBuf,
    #[clap(short = 'o', long, default_value = "-")]
    output: OutFilePath,
}

impl DumpSelectorsFile {
    fn run(&self) -> anyhow::Result<()> {
        let resource_set = sci_resources::file::open_game_resources(&self.root_dir)?;
        let script_loader = ScriptLoader::load_from(&resource_set)?;

        // The output expects an S-Expression like this:
        //
        // (selectors
        //   selector-name <selector-id>
        //   ...)

        let mut selectors_file = self.output.open()?;
        // Note that we leave the next write location at the end of the line,
        // to write the correct closing paren.
        write!(selectors_file, "(selectors")?;

        for selector in script_loader.selectors() {
            write!(selectors_file, "\n  {} {}", selector.name(), selector.id())?;
        }
        writeln!(selectors_file, ")")?;
        selectors_file.flush()?;
        Ok(())
    }
}

// A quick and dirty topological sort of ClassDefs, so that superclasses appear
// before their subclasses.

fn topo_sort<'a>(classes: impl IntoIterator<Item = Class<'a>>) -> Vec<Class<'a>> {
    let mut state = TopoSortState {
        visited: std::collections::HashSet::new(),
        stack: Vec::new(),
    };

    for class in classes {
        state.visit(class);
    }

    state.stack
}

struct TopoSortState<'a> {
    visited: std::collections::HashSet<Species>,
    stack: Vec<Class<'a>>,
}

impl<'a> TopoSortState<'a> {
    fn visit(&mut self, class: Class<'a>) {
        if self.visited.contains(&class.species()) {
            return;
        }
        self.visited.insert(class.species());
        if let Some(super_class) = class.super_class() {
            self.visit(super_class);
        }
        self.stack.push(class);
    }
}

#[derive(Parser)]
struct DumpClassDefFile {
    #[clap(short = 'd')]
    root_dir: PathBuf,
    #[clap(short = 'o', long, default_value = "-")]
    output: OutFilePath,
}

impl DumpClassDefFile {
    fn run(&self) -> anyhow::Result<()> {
        let resource_set = sci_resources::file::open_game_resources(&self.root_dir)?;
        let class_decl_set = ClassDeclSet::new(&resource_set)?;

        let mut class_def_file = self.output.open()?;

        let classes = topo_sort(class_decl_set.classes());

        for class in classes {
            eprintln!("Dumping class {:#?}", class);
            let name = class
                .name()
                .map(ToString::to_string)
                .unwrap_or_else(|| format!("class{}", class.species().num()));

            writeln!(class_def_file, "(classdef {name}")?;
            writeln!(class_def_file, " script# {}", class.script_id().num())?;
            writeln!(class_def_file, " class# {}", class.species().num())?;
            writeln!(
                class_def_file,
                " super# {}",
                class
                    .super_class()
                    .map_or(0xFFFFu16, |cls| cls.species().num())
            )?;
            writeln!(
                class_def_file,
                " file# \"script{}.sc\"\n",
                class.script_id().num()
            )?;

            writeln!(class_def_file, "\t(properties")?;
            for property in class.new_properties() {
                writeln!(
                    class_def_file,
                    "\t\t{} {}",
                    property.name(),
                    property.base_value(),
                )?;
            }
            writeln!(class_def_file, "\t)\n")?;

            writeln!(class_def_file, "\t(methods")?;
            for method in class.methods() {
                writeln!(class_def_file, "\t\t{}", method.name())?;
            }
            writeln!(class_def_file, "\t)")?;
            writeln!(class_def_file, ")\n\n")?;
        }
        Ok(())
    }
}

#[derive(Subcommand)]
enum ScriptCommand {
    #[clap(name = "dump-selectors")]
    DumpSelectorsFile(DumpSelectorsFile),
    #[clap(name = "dump-class-def")]
    DumpClassDefFile(DumpClassDefFile),
}

impl ScriptCommand {
    fn run(&self) -> anyhow::Result<()> {
        match self {
            ScriptCommand::DumpSelectorsFile(dump) => dump.run()?,
            ScriptCommand::DumpClassDefFile(dump) => dump.run()?,
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
