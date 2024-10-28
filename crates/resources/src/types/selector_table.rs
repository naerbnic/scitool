//! Extracts the symbol table from a resource library.
//!
//! The resource table is stored as Vocab:997.

use std::{
    collections::{hash_map, HashMap},
    sync::Arc,
};

use sci_utils::buffer::Buffer;

#[derive(Clone, PartialEq, Eq, Hash)]
struct SharedString(Arc<String>);

impl SharedString {
    fn new(s: String) -> Self {
        Self(Arc::new(s))
    }
    fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for SharedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&*self.0, f)
    }
}

impl std::borrow::Borrow<str> for SharedString {
    fn borrow(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug)]
pub struct SelectorTable {
    entries: Vec<SharedString>,
    reverse_entries: HashMap<SharedString, Vec<u16>>,
}

impl SelectorTable {
    pub fn load_from<'a, B: Buffer<'a, Idx = u16> + Clone>(data: B) -> anyhow::Result<Self> {
        let (selector_offsets, _) = data.clone().read_length_delimited_records::<u16>()?;
        let mut entries = Vec::with_capacity(selector_offsets.len());
        let mut offset_map: HashMap<u16, SharedString> = HashMap::new();

        for selector_offset in selector_offsets {
            let string = match offset_map.entry(selector_offset) {
                hash_map::Entry::Occupied(occupied_entry) => occupied_entry.get().clone(),
                hash_map::Entry::Vacant(vacant_entry) => {
                    let entry_data = data.clone().sub_buffer(selector_offset..);
                    let (string_length, entry_data) = entry_data.read_value::<u16>()?;
                    vacant_entry
                        .insert(SharedString::new(String::from_utf8(
                            entry_data.sub_buffer(..string_length).as_ref().to_vec(),
                        )?))
                        .clone()
                }
            };
            entries.push(string);
        }

        let mut reverse_entries = HashMap::new();

        for (index, string) in entries.iter().enumerate() {
            reverse_entries
                .entry(string.clone())
                .or_insert_with(Vec::new)
                .push(index as u16);
        }
        Ok(Self {
            entries,
            reverse_entries,
        })
    }

    pub fn get_selector_name(&self, index: u16) -> Option<&str> {
        self.entries.get(index as usize).map(|s| s.as_str())
    }

    pub fn get_selector_for_name(&self, name: &str) -> Option<u16> {
        self.reverse_entries
            .get(name)
            .and_then(|v| if v.len() == 1 { Some(v[0]) } else { None })
    }
}
