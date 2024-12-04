use std::collections::BTreeMap;

struct ClassInfo {
    /// The name of the class, for purposes of linking. Must match other
    /// references in the script.
    /// For decompiled scripts, this is the name contained in the classes'
    /// `-name-` property.
    name: String,
    /// Offset of the class in the heap.
    offset: u16,
    /// Species of the class, if any. Will otherwise be computed on linking.
    species: Option<u16>,
}

struct SelectorInfo {
    /// The name of the selector, for purposes of linking.
    name: String,
    /// The offsets where the selector is used in the script.
    script_offsets: Vec<u16>,
    heap_offsets: Vec<u16>,
}

struct SpeciesRelocation {
    /// The name of the species to link.
    name: String,
    script_offsets: Vec<u16>,
    heap_offsets: Vec<u16>,
}

struct ScriptNumRelocation {
    /// The script name to link.
    name: String,
    script_offsets: Vec<u16>,
    heap_offsets: Vec<u16>,
}

pub struct LinkableScript {
    script_name: String,
    script_num: u16,
    num_exports: u16,
    classes: Vec<ClassInfo>,
    selectors: Vec<SelectorInfo>,
    species_relocations: Vec<SpeciesRelocation>,
    script_data: Vec<u8>,
    heap_data: Vec<u8>,
}

pub struct OriginalClassInfo {
    species: u16,
    heap_offset: u16,
    num_variables: u16,
}

pub struct OriginalScriptInfo {
    /// If this is script 0, it is a global script.
    is_global: bool,

    /// This can vary for any script aside from the global script. For the
    /// global script, more locals can be added, but we cannot remove any.
    num_locals: u16,

    /// We can add additional exports, but we cannot remove any for any script.
    num_exports: u16,

    classes: Vec<OriginalClassInfo>,
}

pub struct OriginalSelectorInfo {
    name: String,
}

/// The original game references that must be kept consistent for any old
/// scripts to work, as we can't relink the original game's scripts.
///
/// Information that is in addition to this is linkable, but any changes to
/// the expected values (e.g. which script contains a given class species, or
/// a different index for a selector) will break the original game's scripts.
pub struct OriginalGameInfo {
    scripts: BTreeMap<u16, OriginalScriptInfo>,
    selectors: BTreeMap<u16, OriginalSelectorInfo>,
    max_selector_index: u16,
    max_species: u16,
}

pub struct ScriptSymbols {
    script_num: u16,
    script_name: String,
    
}