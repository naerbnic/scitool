use std::{
    collections::{BTreeMap, BTreeSet, BinaryHeap},
    path::Path,
};

use itertools::Itertools;
use sci_resources::file::ResourceSet;
use scitool_script_loader::{Class, ClassDeclSet, ScriptLoader};

#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct Selector {
    name: String,
    id: u16,
}

impl Selector {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn id(&self) -> u16 {
        self.id
    }
}

fn dump_selectors(resource_set: &ResourceSet) -> anyhow::Result<Vec<Selector>> {
    let script_loader = ScriptLoader::load_from(resource_set)?;
    let mut ordered_selectors = script_loader.selectors().collect::<Vec<_>>();
    ordered_selectors.sort_by_key(|sel| sel.id());

    Ok(ordered_selectors
        .into_iter()
        .map(|sel| Selector {
            name: sel.name().to_string(),
            id: sel.id(),
        })
        .collect())
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

#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct Property {
    name: String,
    base_value: u16,
}

impl Property {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn base_value(&self) -> u16 {
        self.base_value
    }
}

#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct ClassDef {
    name: String,
    script_num: u16,
    species: u16,
    super_class: Option<u16>,

    properties: Vec<Property>,
    methods: Vec<String>,
}

impl ClassDef {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn script_num(&self) -> u16 {
        self.script_num
    }

    pub fn species(&self) -> u16 {
        self.species
    }

    pub fn super_class(&self) -> Option<u16> {
        self.super_class
    }

    pub fn properties(&self) -> &[Property] {
        &self.properties
    }

    pub fn methods(&self) -> &[String] {
        &self.methods
    }
}

fn dump_class_defs(resource_set: &ResourceSet) -> anyhow::Result<Vec<ClassDef>> {
    let class_decl_set = ClassDeclSet::new(resource_set)?;

    let classes = topo_sort(class_decl_set.classes());

    let mut classes_out = Vec::new();

    for class in classes {
        let name = class
            .name()
            .map(ToString::to_string)
            .unwrap_or_else(|| format!("class{}", class.species().num()));

        let script_num = class.script_id().num();
        let species = class.species().num();
        let super_class = class.super_class().map(|cls| cls.species().num());

        let properties = class
            .properties()
            .map(|property| Property {
                name: property.name().to_string(),
                base_value: property.base_value(),
            })
            .collect();

        let methods = class
            .methods()
            .map(|method| method.name().to_string())
            .collect();

        classes_out.push(ClassDef {
            name,
            script_num,
            species,
            super_class,
            properties,
            methods,
        });
    }
    Ok(classes_out)
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct SciScriptExports {
    pub selectors: Vec<Selector>,
    pub class_defs: Vec<ClassDef>,
}

impl SciScriptExports {
    pub fn read_from_resources(root_dir: &Path) -> anyhow::Result<Self> {
        let resource_set = sci_resources::file::open_game_resources(root_dir)?;

        let selectors = dump_selectors(&resource_set)?;

        let class_defs = dump_class_defs(&resource_set)?;

        Ok(SciScriptExports {
            selectors,
            class_defs,
        })
    }

    pub fn selectors(&self) -> &[Selector] {
        &self.selectors
    }

    pub fn class_defs(&self) -> &[ClassDef] {
        &self.class_defs
    }

    pub fn write_selector_header_to(&self, mut out: impl std::io::Write) -> anyhow::Result<()> {
        // Note that we leave the next write location at the end of the line,
        // to write the correct closing paren.
        write!(out, "(selectors")?;

        for selector in &self.selectors {
            write!(out, "\n  {} {}", selector.name(), selector.id())?;
        }
        writeln!(out, ")\n")?;
        Ok(())
    }

    pub fn write_classdef_header_to(&self, mut out: impl std::io::Write) -> anyhow::Result<()> {
        // Note that we leave the next write location at the end of the line,
        // to write the correct closing paren.

        for class in &self.class_defs {
            writeln!(out, "(classdef {}", class.name())?;
            writeln!(out, " script# {}", class.script_num())?;
            writeln!(out, " class# {}", class.species())?;
            writeln!(out, " super# {}", class.super_class().unwrap_or(0xFFFFu16))?;
            writeln!(out, " file# \"script{}.sc\"\n", class.script_num())?;

            writeln!(out, "\t(properties")?;
            for property in class.properties() {
                writeln!(out, "\t\t{} {}", property.name(), property.base_value())?;
            }
            writeln!(out, "\t)\n")?;

            writeln!(out, "\t(methods")?;
            for method in class.methods() {
                writeln!(out, "\t\t{}", method)?;
            }
            writeln!(out, "\t)")?;
            writeln!(out, ")\n\n")?;
        }
        Ok(())
    }
}
