//! Types for defining symbols.
//!
//! Symbols serve two purposes: They are unique values that can be used to tie
//! together different parts of the code, and they are used during debugging to
//! provide the context they were made in.

use std::sync::Arc;

#[derive(Clone)]
pub struct Symbol {
    unique_id: u64,
    name: Option<Arc<String>>,
}

impl std::cmp::PartialEq for Symbol {
    fn eq(&self, other: &Self) -> bool {
        self.unique_id == other.unique_id
    }
}

impl std::cmp::Eq for Symbol {}

impl std::cmp::PartialOrd for Symbol {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl std::cmp::Ord for Symbol {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.unique_id.cmp(&other.unique_id)
    }
}

impl std::hash::Hash for Symbol {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.unique_id.hash(state);
    }
}

impl std::fmt::Debug for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.name {
            Some(name) => write!(f, "[#{}: {}]", self.unique_id, name.as_str()),
            None => write!(f, "[#{}]", self.unique_id),
        }
    }
}

pub struct SymbolFactory {
    next_id: u64,
}

impl SymbolFactory {
    pub fn new() -> Self {
        Self { next_id: 0 }
    }

    pub fn create(&mut self) -> Symbol {
        let next_id = self.next_id;
        self.next_id += 1;
        Symbol {
            unique_id: next_id,
            name: None,
        }
    }

    pub fn create_named(&mut self, name: impl Into<String>) -> Symbol {
        let next_id = self.next_id;
        self.next_id += 1;
        Symbol {
            unique_id: next_id,
            name: Some(Arc::new(name.into())),
        }
    }
}

impl Default for SymbolFactory {
    fn default() -> Self {
        Self::new()
    }
}
