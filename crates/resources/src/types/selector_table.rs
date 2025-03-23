//! Extracts the symbol table from a resource library.
//!
//! The resource table is stored as Vocab:997.

use std::{
    collections::{HashMap, hash_map},
    sync::Arc,
};

use bytes::{Buf, BufMut};
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

struct SelectorInner {
    name: SharedString,
    id: u16,
}

#[derive(Clone)]
pub struct Selector(Arc<SelectorInner>);

impl Selector {
    pub fn name(&self) -> &str {
        self.0.name.as_str()
    }

    pub fn id(&self) -> u16 {
        self.0.id
    }
}

impl std::fmt::Debug for Selector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Selector")
            .field("name", &self.name())
            .field("id", &self.id())
            .finish()
    }
}

#[derive(Clone, Debug)]
pub struct SelectorTable {
    entries: Vec<Selector>,
    reverse_entries: HashMap<SharedString, Vec<Selector>>,
}

impl SelectorTable {
    pub fn load_from<'a, B: Buffer<'a, Idx = u16> + Clone>(data: B) -> anyhow::Result<Self> {
        // A weird property: The number of entries given in Vocab 997 appears to be one
        // _less_ than the actual number of entries.
        let (num_entries_minus_one, entries_table) = data.clone().read_value::<u16>()?;
        let num_entries = num_entries_minus_one + 1;
        let (selector_offsets, _) = entries_table.read_values(num_entries as usize)?;
        let mut entries: HashMap<_, Vec<_>> = HashMap::new();
        let mut offset_map: HashMap<u16, SharedString> = HashMap::new();

        for (id, selector_offset) in selector_offsets.into_iter().enumerate() {
            let name = match offset_map.entry(selector_offset) {
                hash_map::Entry::Occupied(occupied_entry) => occupied_entry.get().clone(),
                hash_map::Entry::Vacant(vacant_entry) => {
                    let entry_data = data.clone().sub_buffer(selector_offset..);
                    let (string_length, entry_data) = entry_data.read_value::<u16>()?;
                    let str_buffer = entry_data.sub_buffer(..string_length);
                    let str_guard = str_buffer.lock();
                    let mut str_data = Vec::with_capacity(str_guard.remaining());
                    str_data.put(str_guard);
                    let name = SharedString::new(String::from_utf8(str_data)?);
                    vacant_entry.insert(name).clone()
                }
            };
            entries
                .entry(name.clone())
                .or_default()
                .push(Selector(Arc::new(SelectorInner {
                    name,
                    id: id.try_into().unwrap(),
                })));
        }

        let entries: Vec<_> = entries
            .into_values()
            .filter_map(|mut v| {
                if v.len() == 1 {
                    Some(v.pop().unwrap())
                } else {
                    None
                }
            })
            .collect();

        let mut reverse_entries = HashMap::new();

        for selector in entries.iter() {
            reverse_entries
                .entry(selector.0.name.clone())
                .or_insert_with(Vec::new)
                .push(selector.clone());
        }
        Ok(Self {
            entries,
            reverse_entries,
        })
    }

    pub fn get_selector_by_id(&self, index: u16) -> Option<&Selector> {
        self.entries.get(index as usize)
    }

    pub fn get_selector_by_name(&self, name: &str) -> Option<&Selector> {
        self.reverse_entries
            .get(name)
            .and_then(|v| if v.len() == 1 { Some(&v[0]) } else { None })
    }

    pub fn selectors(&self) -> impl Iterator<Item = &Selector> {
        self.entries.iter()
    }
}
