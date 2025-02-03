use std::path::PathBuf;

use clap::{Parser, Subcommand};
use itertools::Itertools;
use scitool_script_loader::{Class, ClassDeclSet, ScriptLoader};
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
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
    // Map from species to class objects.
    let class_map = classes
        .into_iter()
        .map(|class| (class.species(), class))
        .collect::<BTreeMap<_, _>>();

    // Map from superclass species to subclass species.
    let subclasses = class_map
        .values()
        .filter_map(|class| {
            class
                .super_class()
                .map(|super_class| (super_class.species(), class.species()))
        })
        .into_group_map();

    let mut pending_classes: BTreeSet<_> = class_map
        .iter()
        .filter_map(|(&c, v)| v.super_class().map(|_| c))
        .collect();

    let mut class_queue = class_map
        .iter()
        .filter_map(|(&c, v)| {
            if v.super_class().is_none() {
                Some(std::cmp::Reverse(c))
            } else {
                None
            }
        })
        .collect::<BinaryHeap<_>>();

    let mut result_classes = Vec::new();

    while let Some(std::cmp::Reverse(next_species)) = class_queue.pop() {
        result_classes.push(class_map[&next_species].clone());
        subclasses
            .get(&next_species)
            .map(|subclasses| &subclasses[..])
            .unwrap_or(&[])
            .iter()
            .filter(|subclass| pending_classes.remove(subclass))
            .for_each(|&subclass| class_queue.push(std::cmp::Reverse(subclass)));
    }

    result_classes
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
            for property in class.properties() {
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
