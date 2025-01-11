use std::collections::BTreeMap;

struct ClassInfo {
    /// The name of the class, for purposes of linking. Must match other
    /// references in the script.
    /// For decompiled scripts, this is the name contained in the classes'
    /// `-name-` property.
    #[expect(dead_code)]
    name: String,
    /// Offset of the class in the heap.
    #[expect(dead_code)]
    offset: u16,
    /// Species of the class, if any. Will otherwise be computed on linking.
    #[expect(dead_code)]
    species: Option<u16>,
}

struct SelectorInfo {
    /// The name of the selector, for purposes of linking.
    #[expect(dead_code)]
    name: String,
    /// The offsets where the selector is used in the script.
    #[expect(dead_code)]
    script_offsets: Vec<u16>,
    #[expect(dead_code)]
    heap_offsets: Vec<u16>,
}

struct SpeciesRelocation {
    /// The name of the species to link.
    #[expect(dead_code)]
    name: String,
    #[expect(dead_code)]
    script_offsets: Vec<u16>,
    #[expect(dead_code)]
    heap_offsets: Vec<u16>,
}

#[expect(dead_code)]
struct ScriptNumRelocation {
    /// The script name to link.
    name: String,
    script_offsets: Vec<u16>,
    heap_offsets: Vec<u16>,
}

pub struct LinkableScript {
    #[expect(dead_code)]
    script_name: String,
    #[expect(dead_code)]
    script_num: u16,
    #[expect(dead_code)]
    num_exports: u16,
    #[expect(dead_code)]
    classes: Vec<ClassInfo>,
    #[expect(dead_code)]
    selectors: Vec<SelectorInfo>,
    #[expect(dead_code)]
    species_relocations: Vec<SpeciesRelocation>,
    #[expect(dead_code)]
    script_data: Vec<u8>,
    #[expect(dead_code)]
    heap_data: Vec<u8>,
}

pub struct OriginalClassInfo {
    #[expect(dead_code)]
    species: u16,
    #[expect(dead_code)]
    heap_offset: u16,
    #[expect(dead_code)]
    num_variables: u16,
}

pub struct OriginalScriptInfo {
    /// If this is script 0, it is a global script.
    #[expect(dead_code)]
    is_global: bool,

    /// This can vary for any script aside from the global script. For the
    /// global script, more locals can be added, but we cannot remove any.
    #[expect(dead_code)]
    num_locals: u16,

    /// We can add additional exports, but we cannot remove any for any script.
    #[expect(dead_code)]
    num_exports: u16,

    #[expect(dead_code)]
    classes: Vec<OriginalClassInfo>,
}

pub struct OriginalSelectorInfo {
    #[expect(dead_code)]
    name: String,
}

/// The original game references that must be kept consistent for any old
/// scripts to work, as we can't relink the original game's scripts.
///
/// Information that is in addition to this is linkable, but any changes to
/// the expected values (e.g. which script contains a given class species, or
/// a different index for a selector) will break the original game's scripts.
pub struct OriginalGameInfo {
    #[expect(dead_code)]
    scripts: BTreeMap<u16, OriginalScriptInfo>,
    #[expect(dead_code)]
    selectors: BTreeMap<u16, OriginalSelectorInfo>,
    #[expect(dead_code)]
    max_selector_index: u16,
    #[expect(dead_code)]
    max_species: u16,
}

pub struct ScriptSymbols {
    #[expect(dead_code)]
    script_num: u16,
    #[expect(dead_code)]
    script_name: String,
}
