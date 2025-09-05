//! Extracts the symbol table from a resource library.
//!
//! The resource table is stored as Vocab:997.

use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap, hash_map},
    fmt::Debug,
    sync::Arc,
};

use crate::{
    script_loader::errors::MalformedDataError,
    utils::{
        errors::NoError,
        mem_reader::{MemReader, NoErrorResultExt as _},
    },
};

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct SharedString(Arc<String>);

impl SharedString {
    fn new(s: String) -> Self {
        Self(Arc::new(s))
    }

    fn from_utf8<'a, B>(bytes: B) -> Result<Self, std::string::FromUtf8Error>
    where
        B: Into<Cow<'a, [u8]>>,
    {
        let string = String::from_utf8(bytes.into().into_owned())?;
        Ok(Self::new(string))
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

#[derive(Clone, Eq, PartialEq, Hash, PartialOrd, Ord)]
struct SelectorInner {
    name: SharedString,
    id: u16,
}

#[derive(Clone, Eq, PartialEq, Hash, PartialOrd, Ord)]
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

#[derive(Debug)]
struct SelectorTableInner {
    entries: BTreeMap<u16, Selector>,
    reverse_entries: HashMap<SharedString, Vec<Selector>>,
}

#[derive(Clone, Debug)]
pub struct SelectorTable(Arc<SelectorTableInner>);

impl SelectorTable {
    pub(crate) fn load_from<M: MemReader<Error = NoError>>(
        data: &M,
    ) -> Result<Self, MalformedDataError> {
        // A weird property: The number of entries given in Vocab 997 appears to be one
        // _less_ than the actual number of entries.

        let mut index_table = data
            .sub_reader_range("Selector index table", ..)
            .map_err(MalformedDataError::map_from("Selector index table"))?;

        let num_entries_minus_one = index_table
            .read_value::<u16>("Selector Count")
            .map_err(MalformedDataError::map_from("Selector Count"))?;
        let num_entries = num_entries_minus_one + 1;
        let selector_offsets = index_table
            .read_values::<u16>("Selector offsets", num_entries.into())
            .map_err(MalformedDataError::map_from("Selector offsets"))?;
        let mut entries: HashMap<_, Vec<_>> = HashMap::new();
        let mut offset_map: HashMap<u16, SharedString> = HashMap::new();

        for (id, selector_offset) in selector_offsets.into_iter().enumerate() {
            let name = match offset_map.entry(selector_offset) {
                hash_map::Entry::Occupied(occupied_entry) => occupied_entry.get().clone(),
                hash_map::Entry::Vacant(vacant_entry) => {
                    let mut entry_data = data
                        .sub_reader_range("Selector entry", usize::from(selector_offset)..)
                        .map_err(MalformedDataError::map_from("Selector entry"))?;
                    let string_length = entry_data
                        .read_value::<u16>("Selector string length")
                        .map_err(MalformedDataError::map_from("Selector string length"))?;
                    let mut entry_buffer = entry_data
                        .sub_reader_range("Selector string data", ..usize::from(string_length))
                        .map_err(MalformedDataError::map_from("Selector string data"))?;
                    let name =
                        SharedString::from_utf8(entry_buffer.read_remaining().remove_no_error())
                            .map_err(MalformedDataError::map_from("Expected valid utf-8 string"))?;
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

        let entries: BTreeMap<_, _> = entries
            .into_values()
            .filter_map(|mut v| {
                if v.len() == 1 {
                    let item = v.pop().unwrap();
                    Some((item.0.id, item))
                } else {
                    None
                }
            })
            .collect();

        let mut reverse_entries = HashMap::new();

        for selector in &entries {
            reverse_entries
                .entry(selector.1.0.name.clone())
                .or_insert_with(Vec::new)
                .push(selector.1.clone());
        }
        Ok(Self(Arc::new(SelectorTableInner {
            entries,
            reverse_entries,
        })))
    }

    pub fn get_selector_by_id(&self, index: u16) -> Option<&Selector> {
        self.0.entries.get(&index)
    }

    pub fn get_selector_by_name(&self, name: &str) -> Option<&Selector> {
        self.0
            .reverse_entries
            .get(name)
            .and_then(|v| if v.len() == 1 { Some(&v[0]) } else { None })
    }

    pub fn selectors(&self) -> impl Iterator<Item = &Selector> {
        self.0.entries.values()
    }
}
