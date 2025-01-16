use std::collections::HashMap;

use mem_loader::LoadedScript;
use sci_resources::{file::ResourceSet, ResourceType};
use sci_utils::buffer::Buffer;

mod mem_loader;
mod selectors;

pub use mem_loader::Object;

const SELECTOR_TABLE_VOCAB_NUM: u16 = 997;

#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ScriptId(u16);

impl ScriptId {
    pub fn num(self) -> u16 {
        self.0
    }
}

impl std::fmt::Debug for ScriptId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::write(f, format_args!("#{}", self.0))
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Species(u16);

impl Species {
    pub fn num(self) -> u16 {
        self.0
    }
}

impl std::fmt::Debug for Species {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::write(f, format_args!("[{}]", self.0))
    }
}

pub struct ScriptLoader {
    selectors: selectors::SelectorTable,
    loaded_scripts: HashMap<ScriptId, LoadedScript>,
}

impl ScriptLoader {
    pub fn load_from(resources: &ResourceSet) -> anyhow::Result<Self> {
        let selector_table_data = resources
            .get_resource(&sci_resources::ResourceId::new(
                ResourceType::Vocab,
                SELECTOR_TABLE_VOCAB_NUM,
            ))
            .ok_or_else(|| anyhow::anyhow!("Selector table not found"))?
            .load_data()?;
        let selectors = selectors::SelectorTable::load_from(selector_table_data.narrow())?;
        let mut loaded_scripts = HashMap::new();
        for script in resources.resources_of_type(ResourceType::Script) {
            let script_num = script.id().resource_num();
            let script_data = script.load_data()?;
            let heap = resources
                .get_resource(&sci_resources::ResourceId::new(
                    ResourceType::Heap,
                    script_num,
                ))
                .ok_or_else(|| anyhow::anyhow!("Heap not found for script {}", script_num))?
                .load_data()?;

            let loaded_script =
                mem_loader::LoadedScript::load(&selectors, &script_data.narrow(), &heap.narrow())?;

            loaded_scripts.insert(ScriptId(script_num), loaded_script);
        }

        Ok(Self {
            selectors,
            loaded_scripts,
        })
    }

    pub fn script_ids(&self) -> impl Iterator<Item = ScriptId> + '_ {
        self.loaded_scripts.keys().copied()
    }

    pub fn selectors(&self) -> impl Iterator<Item = &selectors::Selector> {
        self.selectors.selectors()
    }

    pub fn loaded_scripts(&self) -> impl Iterator<Item = (ScriptId, &LoadedScript)> {
        self.loaded_scripts.iter().map(|(id, script)| (*id, script))
    }
}

pub struct ClassDeclSet {
    classes: HashMap<Species, ClassData>,
}

impl ClassDeclSet {
    pub fn new(resources: &ResourceSet) -> anyhow::Result<Self> {
        let loader = ScriptLoader::load_from(resources)?;
        let mut classes = HashMap::new();
        for (script_id, loaded_script) in loader.loaded_scripts() {
            for object in loaded_script.objects() {
                if !object.is_class() {
                    continue;
                }

                let class = ClassData::with_object(script_id, object);
                classes.insert(class.species, class);
            }
        }

        Ok(Self { classes })
    }

    pub fn classes(&self) -> impl Iterator<Item = Class> {
        self.classes.values().map(|data| Class { root: self, data })
    }
}

#[derive(Clone)]
pub struct Class<'a> {
    root: &'a ClassDeclSet,
    data: &'a ClassData,
}

impl std::fmt::Debug for Class<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Class")
            .field("name", &self.data.name)
            .field("script_id", &self.data.script_id)
            .field("species", &self.data.species)
            .field("super_class", &self.data.super_class)
            .field("methods", &self.methods())
            .field("properties", &self.new_properties())
            .finish()
    }
}

impl<'a> Class<'a> {
    pub fn name(&self) -> Option<&str> {
        self.data.name.as_deref()
    }

    pub fn script_id(&self) -> ScriptId {
        self.data.script_id
    }

    pub fn species(&self) -> Species {
        self.data.species
    }

    pub fn super_class(&self) -> Option<Class<'a>> {
        self.data.super_class.map(|super_class| Class {
            root: self.root,
            data: &self.root.classes[&super_class],
        })
    }

    pub fn get_method(&self, name: &str) -> Option<&Method> {
        self.data.methods.get(name)
    }

    pub fn get_property(&self, name: &str) -> Option<&Property> {
        self.data.properties.get(name)
    }

    pub fn methods(&self) -> impl Iterator<Item = &Method> + std::fmt::Debug {
        self.data.methods.values()
    }

    pub fn new_methods(&self) -> impl Iterator<Item = &Method> + std::fmt::Debug {
        let super_class = self.super_class();
        self.data.methods.values().filter(move |method| {
            if let Some(super_class) = &super_class {
                super_class.get_method(&method.name).is_none()
            } else {
                true
            }
        })
    }

    pub fn properties(&self) -> impl Iterator<Item = &Property> + std::fmt::Debug {
        self.data.properties.values()
    }

    pub fn new_properties(&self) -> impl Iterator<Item = &Property> + std::fmt::Debug {
        let super_class = self.super_class();
        self.data.properties.values().filter(move |property| {
            if let Some(super_class) = &super_class {
                if let Some(super_property) = super_class.get_property(&property.name) {
                    super_property.base_value() != property.base_value()
                } else {
                    true
                }
            } else {
                true
            }
        })
    }
}

struct ClassData {
    name: Option<String>,
    script_id: ScriptId,
    species: Species,
    super_class: Option<Species>,
    methods: HashMap<String, Method>,
    properties: HashMap<String, Property>,
}

impl ClassData {
    fn with_object(script_id: ScriptId, object: &Object) -> Self {
        let species_id = object.species();
        let super_class_id = object.super_class();

        assert!(species_id != 0xFFFF);

        let mut methods = HashMap::new();
        let mut properties = HashMap::new();

        for (prop_selector, base_value) in object.properties() {
            properties.insert(
                prop_selector.name().to_string(),
                Property {
                    name: prop_selector.name().to_string(),
                    base_value,
                },
            );
        }

        for method_selector in object.methods() {
            methods.insert(
                method_selector.name().to_string(),
                Method {
                    name: method_selector.name().to_string(),
                },
            );
        }

        Self {
            name: object.name().map(|s| s.to_owned()),
            script_id,
            species: Species(species_id),
            super_class: if super_class_id == 0xFFFF {
                None
            } else {
                Some(Species(super_class_id))
            },
            methods,
            properties,
        }
    }
}

#[derive(Debug)]
pub struct Method {
    name: String,
}

impl Method {
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug)]
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
